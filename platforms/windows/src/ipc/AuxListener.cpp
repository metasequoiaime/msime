#include "AuxListener.h"
#include "PipeIo.h"
#include "PipeListener.h"
#include "PipePeer.h"

namespace msime::windows {
namespace {
// One message per connection, read with a short deadline so a client that
// connects and never writes cannot hold the single instance.
constexpr DWORD aux_read_timeout_ms = 200;
constexpr DWORD aux_accept_slice_ms = 1000;
constexpr DWORD aux_backoff_ms = 20;
} // namespace

std::unique_ptr<AuxListener> AuxListener::create(const std::wstring &name,
                                                 Sink sink, DWORD &error,
                                                 MessageSink message_sink,
                                                 ActivationSink activation,
                                                 TerminalSink terminal,
                                                 MaintenanceSink maintenance,
                                                 StatisticsSink statistics,
                                                 KeysSink keys,
                                                 KeySoundSink key_sound) {
  error = ERROR_SUCCESS;
  if (!sink) {
    error = ERROR_INVALID_PARAMETER;
    return nullptr;
  }
  auto listener = PipeListener::create(name, error);
  if (!listener)
    return nullptr;
  std::unique_ptr<AuxListener> aux(new AuxListener());
  aux->cancel_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (!aux->cancel_) {
    error = GetLastError();
    return nullptr;
  }
  aux->listener_ = std::move(listener);
  aux->sink_ = std::move(sink);
  aux->message_sink_ = std::move(message_sink);
  aux->activation_ = std::move(activation);
  aux->terminal_ = std::move(terminal);
  aux->maintenance_ = std::move(maintenance);
  aux->statistics_ = std::move(statistics);
  aux->keys_ = std::move(keys);
  aux->key_sound_ = std::move(key_sound);
  aux->worker_ = std::thread([raw = aux.get()] { raw->run(); });
  return aux;
}

AuxListener::~AuxListener() {
  stop();
  if (cancel_)
    CloseHandle(cancel_);
}

void AuxListener::request_stop() {
  if (cancel_)
    SetEvent(cancel_);
}

void AuxListener::stop() {
  std::lock_guard<std::mutex> lock(stop_mutex_);
  request_stop();
  if (worker_.joinable())
    worker_.join();
}

AuxStats AuxListener::stats() const {
  std::lock_guard<std::mutex> lock(stats_mutex_);
  return stats_;
}

void AuxListener::callback_failed() noexcept {
  std::lock_guard<std::mutex> lock(stats_mutex_);
  ++stats_.callback_failures;
}

void AuxListener::write_ok(HANDLE connection) {
  // The wire carries UTF-16LE, so this is the two code units of "OK".
  static constexpr wchar_t ok[] = L"OK";
  const std::vector<uint8_t> reply(
      reinterpret_cast<const uint8_t *>(ok),
      reinterpret_cast<const uint8_t *>(ok) + sizeof(wchar_t) * 2);
  (void)write_frame(connection, reply, aux_read_timeout_ms, cancel_);
}
void AuxListener::run() {
  while (WaitForSingleObject(cancel_, 0) != WAIT_OBJECT_0) {
    auto accepted = listener_->accept(aux_accept_slice_ms, cancel_);
    if (accepted.io.status == IoStatus::Timeout)
      continue;
    if (accepted.io.status == IoStatus::Cancelled)
      return;
    if (!accepted.io.complete() || !accepted.connection) {
      // A client can disappear before accept finishes; that is ordinary, so
      // back off briefly rather than treating it as a hard failure.
      if (accepted.io.system_error == ERROR_NO_DATA ||
          accepted.io.system_error == ERROR_PIPE_NOT_CONNECTED) {
        if (WaitForSingleObject(cancel_, aux_backoff_ms) != WAIT_TIMEOUT)
          return;
        continue;
      }
      // Anything else would spin; latch it and leave the endpoint closed.
      failure_.store(accepted.io.system_error ? accepted.io.system_error
                                              : ERROR_GEN_FAILURE);
      return;
    }
    {
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.accepted;
    }
    const auto message =
        read_message(accepted.connection->handle(),
                     static_cast<DWORD>(max_aux_message_bytes),
                     aux_read_timeout_ms, cancel_);
    if (message.status == IoStatus::Cancelled)
      return;
    if (!message.complete()) {
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.malformed;
      continue;
    }
    const auto text =
        aux_text_from_bytes(message.frame.data(), message.frame.size());
    if (!text) {
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.malformed;
      continue;
    }
    // The DACL has to admit AppContainer and low-integrity hosts, because the
    // TIP runs inside them. The verbs that restart the Server or drop every
    // session are only ever sent by the settings process, so they are
    // accepted from a full desktop process of this user and nothing else.
    const auto maintenance = parse_aux_dictionary_maintenance(*text);
    if (maintenance || *text == L"RestartServer") {
      DWORD peer_error = ERROR_SUCCESS;
      if (!pipe_client_is_desktop_user(accepted.connection->handle(),
                                       peer_error)) {
        std::lock_guard<std::mutex> lock(stats_mutex_);
        ++stats_.rejected;
        continue;
      }
    }
    try {
      if (message_sink_)
        message_sink_(*text);
    } catch (...) {
      callback_failed();
    }
    if (const auto activation = parse_aux_activation(*text)) {
      try {
        if (activation_)
          activation_(*activation);
      } catch (...) {
        callback_failed();
      }
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.dispatched;
      continue;
    }
    if (maintenance) {
      // Same contract as the deactivation below: the caller takes "OK" as
      // proof that the sessions are gone and the dictionary lock is free, so
      // it is written only once that is actually true.
      bool done = false;
      try {
        done = maintenance_ && maintenance_(*maintenance);
      } catch (...) {
        callback_failed();
      }
      if (done)
        write_ok(accepted.connection->handle());
      std::lock_guard<std::mutex> lock(stats_mutex_);
      if (done)
        ++stats_.dispatched;
      else
        ++stats_.unknown_verb;
      continue;
    }
    if (const auto terminal = parse_aux_terminal_deactivation(*text)) {
      // The client id is (pid << 32) | tid; a sender may only fence its own.
      ULONG pid = 0;
      DWORD peer_error = ERROR_SUCCESS;
      if (!pipe_client_in_session(accepted.connection->handle(), pid,
                                  peer_error) ||
          static_cast<DWORD>(terminal->client_id >> 32) != pid) {
        std::lock_guard<std::mutex> lock(stats_mutex_);
        ++stats_.rejected;
        continue;
      }
      // The DLL polls this pipe for a literal "OK" and blocks its TSF thread
      // for 150 ms without one. Answer only once the client really is gone:
      // an unconditional "OK" would tell the DLL a teardown happened that did
      // not, which is worse than the wait.
      bool done = false;
      try {
        done = terminal_ && terminal_(*terminal);
      } catch (...) {
        callback_failed();
      }
      if (done)
        write_ok(accepted.connection->handle());
      std::lock_guard<std::mutex> lock(stats_mutex_);
      if (done)
        ++stats_.dispatched;
      else
        ++stats_.unknown_verb;
      continue;
    }
    if (const auto statistics = parse_aux_typing_statistics(*text)) {
      // The batch carries typed characters, so it goes to the sink and nowhere else. The DLL backs off when no "OK" arrives, which is the right answer both when statistics are off and when nobody is listening for them.
      bool done = false;
      try {
        done = statistics_ && statistics_(*statistics);
      } catch (...) {
        callback_failed();
      }
      if (done)
        write_ok(accepted.connection->handle());
      std::lock_guard<std::mutex> lock(stats_mutex_);
      if (done)
        ++stats_.dispatched;
      else
        ++stats_.unknown_verb;
      continue;
    }
    if (const auto keys = parse_aux_typing_keys(*text)) {
      // Counts per key only, but still the user's typing: they go to the sink and nowhere else, and silence is the answer whenever statistics are off.
      bool done = false;
      try {
        done = keys_ && keys_(*keys);
      } catch (...) {
        callback_failed();
      }
      if (done)
        write_ok(accepted.connection->handle());
      std::lock_guard<std::mutex> lock(stats_mutex_);
      if (done)
        ++stats_.dispatched;
      else
        ++stats_.unknown_verb;
      continue;
    }
    if (const auto key = parse_aux_key_sound(*text)) {
      // 和 TerminalDeactivation 一样，client 是 (pid << 32) | tid，发送方只能替自己的线程报键。
      ULONG pid = 0;
      DWORD peer_error = ERROR_SUCCESS;
      if (!pipe_client_in_session(accepted.connection->handle(), pid, peer_error) ||
          static_cast<DWORD>(key->client_id >> 32) != pid) {
        std::lock_guard<std::mutex> lock(stats_mutex_);
        ++stats_.rejected;
        continue;
      }
      bool wanted = false;
      try {
        wanted = key_sound_ && key_sound_(*key);
      } catch (...) {
        callback_failed();
      }
      if (wanted)
        write_ok(accepted.connection->handle());
      std::lock_guard<std::mutex> lock(stats_mutex_);
      if (wanted)
        ++stats_.dispatched;
      else
        ++stats_.unknown_verb;
      continue;
    }
    const auto click = parse_aux_langbar_right_click(*text);
    if (!click) {
      // The other Aux verbs are not this listener's business; drop them without
      // acknowledgement rather than pretending to have acted on them.
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.unknown_verb;
      continue;
    }
    {
      std::lock_guard<std::mutex> lock(stats_mutex_);
      ++stats_.dispatched;
    }
    try {
      sink_(tray_menu_anchor(*click));
    } catch (...) {
      callback_failed();
    }
  }
}
} // namespace msime::windows
