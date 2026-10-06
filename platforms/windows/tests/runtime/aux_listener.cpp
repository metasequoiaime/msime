#include "AuxListener.h"
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstring>
#include <iostream>
#include <map>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>
#include <functional>

using namespace msime::windows;
namespace {
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Aux listener test failed at line " +
                             std::to_string(line));
}
std::wstring isolated_name(int serial) {
  return L"\\\\.\\pipe\\MSIMEClientAuxTest-" +
         std::to_wstring(GetCurrentProcessId()) + L"-" +
         std::to_wstring(serial);
}
// Write exactly what the TSF DLL writes: raw code units, no NUL terminator.
bool send_like_tsf(const std::wstring &name, const std::wstring &message) {
  for (int attempt = 0; attempt < 50; ++attempt) {
    HANDLE pipe = CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0,
                              nullptr, OPEN_EXISTING,
                              SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                              nullptr);
    if (pipe == INVALID_HANDLE_VALUE) {
      Sleep(20);
      continue;
    }
    DWORD written = 0;
    const DWORD size = static_cast<DWORD>(message.size() * sizeof(wchar_t));
    const BOOL ok = WriteFile(pipe, message.data(), size, &written, nullptr);
    CloseHandle(pipe);
    return ok && written == size;
  }
  return false;
}
// The DLL does not just write: after a TerminalDeactivation it waits on the
// same handle for a literal "OK". Reading the reply back is the only way to
// tell an acknowledged teardown from a silently dropped one.
std::wstring send_and_read_reply(const std::wstring &name,
                                 const std::wstring &message) {
  for (int attempt = 0; attempt < 50; ++attempt) {
    HANDLE pipe = CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0,
                              nullptr, OPEN_EXISTING,
                              SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                              nullptr);
    if (pipe == INVALID_HANDLE_VALUE) {
      Sleep(20);
      continue;
    }
    DWORD written = 0;
    const DWORD size = static_cast<DWORD>(message.size() * sizeof(wchar_t));
    if (!WriteFile(pipe, message.data(), size, &written, nullptr) ||
        written != size) {
      CloseHandle(pipe);
      return {};
    }
    wchar_t reply[8]{};
    DWORD read = 0;
    const BOOL ok = ReadFile(pipe, reply, sizeof(reply), &read, nullptr);
    CloseHandle(pipe);
    if (!ok || read < sizeof(wchar_t))
      return {};
    return std::wstring(reply, read / sizeof(wchar_t));
  }
  return {};
}
// The DLL writes one message and closes immediately, so the Server must already
// be parked in accept() to catch it; otherwise ConnectNamedPipe reports
// ERROR_NO_DATA and the message is lost. A long-running Server satisfies that,
// but a just-created listener in a test may not yet have reached accept(), so
// retry until the endpoint is actually listening.
bool deliver(const std::wstring &name, const std::wstring &message,
             const std::function<uint64_t()> &progress, uint64_t target) {
  for (int attempt = 0; attempt < 100; ++attempt) {
    if (!send_like_tsf(name, message))
      return false;
    for (int spin = 0; spin < 10; ++spin) {
      if (progress() >= target)
        return true;
      Sleep(10);
    }
  }
  return false;
}
class Collected final {
public:
  void add(const TrayMenuAnchor &anchor) {
    std::lock_guard<std::mutex> lock(mutex_);
    anchors_.push_back(anchor);
    ready_.notify_all();
  }
  bool wait_for(size_t count) {
    std::unique_lock<std::mutex> lock(mutex_);
    return ready_.wait_for(lock, std::chrono::seconds(5),
                           [&] { return anchors_.size() >= count; });
  }
  std::vector<TrayMenuAnchor> snapshot() {
    std::lock_guard<std::mutex> lock(mutex_);
    return anchors_;
  }

private:
  std::mutex mutex_;
  std::condition_variable ready_;
  std::vector<TrayMenuAnchor> anchors_;
};
} // namespace
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    {
      // A client byte-identical to the TSF DLL reaches the sink.
      const auto name = isolated_name(1);
      Collected collected;
      DWORD error = ERROR_SUCCESS;
      std::atomic<int> activations{0};
      std::atomic<int> deactivations{0};
      std::atomic<bool> throw_activation{false};
      // The terminal sink stands in for the Server's deactivation path. It
      // records what it was asked and answers what the test tells it to, so
      // both outcomes can be checked at the wire.
      std::atomic<bool> deactivated{false};
      std::atomic<uint64_t> seen_client{0};
      std::atomic<uint64_t> seen_token{0};
      std::atomic<uint64_t> terminal_calls{0};
      std::atomic<bool> maintenance_ok{false};
      std::atomic<uint64_t> quiesces{0};
      std::atomic<uint64_t> resumes{0};
      std::atomic<bool> statistics_ok{false};
      std::mutex statistics_mutex;
      std::vector<AuxTypingStatistics> statistics_batches;
      std::atomic<bool> keys_ok{false};
      std::vector<AuxTypingKeys> key_batches;
      auto listener = AuxListener::create(
          name, [&](const TrayMenuAnchor &a) { collected.add(a); }, error, {},
          [&](AuxActivation activation) {
            if (throw_activation.load())
              throw std::runtime_error("Synthetic activation callback failure");
            if (activation == AuxActivation::Activated)
              ++activations;
            else
              ++deactivations;
          },
          [&](const AuxTerminalDeactivation &terminal) {
            seen_client.store(terminal.client_id);
            seen_token.store(terminal.focus_token);
            ++terminal_calls;
            return deactivated.load();
          },
          [&](AuxDictionaryMaintenance request) {
            if (request == AuxDictionaryMaintenance::Quiesce)
              ++quiesces;
            else
              ++resumes;
            return maintenance_ok.load();
          },
          [&](const AuxTypingStatistics &batch) {
            std::lock_guard<std::mutex> lock(statistics_mutex);
            statistics_batches.push_back(batch);
            return statistics_ok.load();
          },
          [&](const AuxTypingKeys &batch) {
            std::lock_guard<std::mutex> lock(statistics_mutex);
            key_batches.push_back(batch);
            return keys_ok.load();
          });
      require(listener != nullptr);
      const auto dispatched = [&] { return listener->stats().dispatched; };
      require(deliver(name, L"LangbarRightClick|100|200|140|240", dispatched, 1));
      require(collected.wait_for(1));
      const auto anchors = collected.snapshot();
      require(anchors[0].center_x == 120 && anchors[0].top == 200);
      require(listener->stats().dispatched == 1);

      // Several sequential clients, proving accept/read/close rollover.
      for (uint64_t index = 0; index < 5; ++index)
        require(deliver(name, L"LangbarRightClick|0|0|40|40", dispatched,
                        2 + index));
      require(collected.wait_for(6));
      require(listener->stats().dispatched == 6);

      // The activation edges reach their own sink rather than being dropped.
      // Gating the toolbar on the mode view instead made it blink away on any
      // temporary focus suspension.
      // The sink runs before the dispatch counter advances, so once deliver
      // observes the count the callback has already been seen.
      require(deliver(name, L"IMEDeactivation", dispatched, 7));
      require(deactivations.load() == 1 && activations.load() == 0);
      require(deliver(name, L"IMEActivation", dispatched, 8));
      require(activations.load() == 1);
      throw_activation.store(true);
      require(deliver(name, L"IMEActivation", dispatched, 9));
      require(listener->stats().callback_failures == 1);
      throw_activation.store(false);
      require(deliver(name, L"LangbarRightClick|5|5|45|45", dispatched, 10));
      require(collected.wait_for(7));
      require(listener->stats().unknown_verb == 0);

      // A genuinely unknown verb is still counted and dropped, and the
      // endpoint keeps working afterwards.
      require(deliver(name, L"SomethingElse|1",
                      [&] { return listener->stats().unknown_verb; }, 1));
      require(listener->stats().unknown_verb == 1);
      // A deactivation that did not happen must not be acknowledged: the DLL
      // would take an "OK" as proof of a teardown. It counts as unhandled and
      // nothing is written back.
      // The client id is the DLL's real (pid << 32) | tid, which does not
      // fit in 32 bits, and the listener checks the pid against the sender.
      const uint64_t own_client =
          (static_cast<uint64_t>(GetCurrentProcessId()) << 32) |
          GetCurrentThreadId();
      const auto terminal_message = [](uint64_t client, uint64_t token) {
        return L"TerminalDeactivation|" + std::to_wstring(client) + L"|" +
               std::to_wstring(token);
      };
      const uint64_t large_token = 18446744073709551615ull;
      deactivated.store(false);
      require(
          send_and_read_reply(name, terminal_message(own_client, large_token))
              .empty());
      require(deliver(name, terminal_message(own_client, large_token),
                      [&] { return listener->stats().unknown_verb; }, 3));
      // The sink is told exactly which client and focus token the DLL named.
      require(seen_client.load() == own_client &&
              seen_token.load() == large_token);

      // Once the client really is gone, the "OK" the DLL is polling for is
      // written back on the same connection, so it stops waiting out its
      // 150 ms.
      deactivated.store(true);
      const auto before = terminal_calls.load();
      require(send_and_read_reply(name, terminal_message(own_client, 11)) ==
              L"OK");
      require(terminal_calls.load() > before);
      require(seen_client.load() == own_client && seen_token.load() == 11);
      // An acknowledged deactivation is dispatched work, not an unknown verb.
      require(listener->stats().unknown_verb == 3);
      // A client id naming another process is refused before the sink runs,
      // so one host cannot fence another's focus.
      const uint64_t foreign_client =
          (static_cast<uint64_t>(GetCurrentProcessId() + 4) << 32) | 1;
      const auto calls_before_forgery = terminal_calls.load();
      require(send_and_read_reply(name, terminal_message(foreign_client, 11))
                  .empty());
      require(listener->stats().rejected == 1);
      require(terminal_calls.load() == calls_before_forgery);
      require(listener->stats().unknown_verb == 3);
      // Dictionary maintenance: the settings process takes "OK" as permission
      // to open the dictionaries exclusively, so a release that did not happen
      // must not be answered.
      maintenance_ok.store(false);
      require(send_and_read_reply(name, L"DictionaryQuiesce").empty());
      require(quiesces.load() == 1);
      maintenance_ok.store(true);
      require(send_and_read_reply(name, L"DictionaryQuiesce") == L"OK");
      require(quiesces.load() == 2);
      // Resume is answered the same way and is routed as its own verb, not
      // confused with the quiesce that preceded it.
      require(send_and_read_reply(name, L"DictionaryResume") == L"OK");
      require(resumes.load() == 1 && quiesces.load() == 2);
      // Passthrough statistics: a refused batch gets no "OK", which is what makes the DLL back off while statistics are switched off.
      require(send_and_read_reply(name, L"TypingStatistics|E|ab").empty());
      statistics_ok.store(true);
      require(send_and_read_reply(name, L"TypingStatistics|C|12") == L"OK");
      {
        std::lock_guard<std::mutex> lock(statistics_mutex);
        require(statistics_batches.size() == 2);
        require(statistics_batches[0].english &&
                statistics_batches[0].characters == L"ab");
        require(!statistics_batches[1].english &&
                statistics_batches[1].characters == L"12");
      }
      // Key heatmap counts: unanswered while statistics are off, which is what keeps the DLL from buffering; a probe and a batch are both routed to the keys sink, never to the character one.
      require(send_and_read_reply(name, L"TypingKeys|2026-10-01|").empty());
      keys_ok.store(true);
      require(send_and_read_reply(name, L"TypingKeys|2026-10-01|") == L"OK");
      require(send_and_read_reply(name, L"TypingKeys|2026-10-01|KeyA=3,Space=2") == L"OK");
      require(send_and_read_reply(name, L"TypingKeys|2026-10-01|KeyA=0").empty());
      {
        std::lock_guard<std::mutex> lock(statistics_mutex);
        require(statistics_batches.size() == 2);
        require(key_batches.size() == 3);
        require(key_batches[0].counts.empty() && key_batches[1].counts.empty());
        require(key_batches[2].day == L"2026-10-01" &&
                key_batches[2].counts ==
                    std::map<std::wstring, uint64_t>{{L"KeyA", 3}, {L"Space", 2}});
      }
      // Progress is the click count, not `dispatched`: acknowledged verbs have already pushed that counter past any fixed target, which would turn deliver's retry into a single send that races the listener's next accept.
      const auto clicks = [&] { return uint64_t(collected.snapshot().size()); };
      require(deliver(name, L"LangbarRightClick|10|10|50|50", clicks, 7));
      require(collected.wait_for(7));

      // A client that connects and never writes must not wedge the endpoint.
      HANDLE idle = CreateFileW(name.c_str(), GENERIC_READ | GENERIC_WRITE, 0,
                                nullptr, OPEN_EXISTING, 0, nullptr);
      require(idle != INVALID_HANDLE_VALUE);
      CloseHandle(idle);
      require(deliver(name, L"LangbarRightClick|20|20|60|60", clicks, 8));
      require(collected.wait_for(8));

      // Stopping twice is safe, and no hard failure was latched.
      listener->stop();
      listener->stop();
      require(listener->failure() == ERROR_SUCCESS);
    }
    {
      // The name is claimed exclusively, so a second Server cannot silently
      // share the endpoint.
      const auto name = isolated_name(2);
      DWORD first_error = ERROR_SUCCESS;
      auto first = AuxListener::create(
          name, [](const TrayMenuAnchor &) {}, first_error);
      require(first != nullptr);
      DWORD second_error = ERROR_SUCCESS;
      auto second = AuxListener::create(
          name, [](const TrayMenuAnchor &) {}, second_error);
      require(second == nullptr);
      first->stop();
    }
    {
      // A sink may ask the listener to stop without deadlocking.
      const auto name = isolated_name(3);
      std::atomic<bool> seen{false};
      DWORD error = ERROR_SUCCESS;
      AuxListener *self = nullptr;
      auto listener = AuxListener::create(
          name,
          [&](const TrayMenuAnchor &) {
            seen.store(true);
            if (self)
              self->request_stop();
          },
          error);
      require(listener != nullptr);
      self = listener.get();
      require(deliver(name, L"LangbarRightClick|1|1|41|41",
                      [&] { return seen.load() ? 1u : 0u; }, 1));
      for (int spin = 0; spin < 250 && !seen.load(); ++spin)
        Sleep(20);
      require(seen.load());
      listener->stop();
    }
    std::cout << "Aux listener: langbar clicks dispatched, endpoint resilient\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << "\n";
    return 1;
  }
}
