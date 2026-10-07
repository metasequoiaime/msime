// msime-voice-local: on-device recognition in its own process.
//
// An input method process is the worst place to hold a speech model: it is loaded into every app that takes text (macOS IMK, fcitx5), a crash there takes typing down with it, and hundreds of megabytes resident in it count against whatever the host was already allowed. Hosts spawn this helper instead and talk to it over stdin/stdout, one JSON object per line. The protocol is documented in shared/voice/README.md.
//
// `msime-voice-local --model <dir> --wav <file>` transcribes a 16 kHz mono 16-bit WAV file and prints the text, for tests and for checking an installed model by hand.

#include "LocalAsr.h"
#include "LocalAsrCommandQueue.h"
#include "VoiceProviders.h"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <filesystem>
#include <iostream>
#include <memory>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <vector>
#if !defined(_WIN32)
#include <poll.h>
#include <unistd.h>
#endif

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif

namespace {

using msime::voice::LocalAsrOptions;
using msime::voice::LocalAsrSession;

constexpr std::size_t kMaxWavBytes = 44 + msime::voice::local_asr_sample_limit * 2;
constexpr std::size_t kMaxRequestLineBytes = 1024 * 1024;
constexpr std::size_t kMaxQueuedRequestBytes = 8 * 1024 * 1024;

std::mutex output_mutex;

void emit(const nlohmann::json &message) {
  const auto line = message.dump(-1, ' ', false, nlohmann::json::error_handler_t::replace);
  std::lock_guard<std::mutex> lock(output_mutex);
  std::fwrite(line.data(), 1, line.size(), stdout);
  std::fputc('\n', stdout);
  std::fflush(stdout);
}

std::optional<std::vector<float>> decode_pcm16(const std::string &encoded) {
  static const auto table = [] {
    std::array<int, 256> values{};
    values.fill(-1);
    const char *alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (int i = 0; i < 64; ++i)
      values[static_cast<unsigned char>(alphabet[i])] = i;
    return values;
  }();
  std::vector<unsigned char> bytes;
  bytes.reserve(encoded.size() * 3 / 4);
  int buffer = 0;
  int bits = 0;
  for (const char character : encoded) {
    if (character == '=')
      break;
    const int value = table[static_cast<unsigned char>(character)];
    if (value < 0)
      return std::nullopt;
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push_back(static_cast<unsigned char>((buffer >> bits) & 0xFF));
    }
  }
  if (bytes.size() % 2 != 0)
    return std::nullopt;
  std::vector<float> samples(bytes.size() / 2);
  for (std::size_t i = 0; i < samples.size(); ++i) {
    const auto sample = static_cast<int16_t>(static_cast<uint16_t>(bytes[2 * i]) | static_cast<uint16_t>(bytes[2 * i + 1]) << 8);
    samples[i] = static_cast<float>(sample) / 32768.0f;
  }
  return samples;
}

std::vector<float> read_wav(const std::string &path) {
  std::ifstream input(path, std::ios::binary);
  if (!input)
    throw std::runtime_error("cannot open " + path);
  std::error_code error;
  const auto length = std::filesystem::file_size(path, error);
  if (error)
    throw std::runtime_error("cannot stat " + path);
  if (length > kMaxWavBytes)
    throw std::runtime_error(path + " is too large for local recognition");
  std::vector<char> data(static_cast<std::size_t>(length));
  if (!data.empty() && !input.read(data.data(), static_cast<std::streamsize>(data.size())))
    throw std::runtime_error("cannot read " + path);
  auto u16 = [&](std::size_t at) { return static_cast<uint16_t>(static_cast<unsigned char>(data[at]) | static_cast<unsigned char>(data[at + 1]) << 8); };
  auto u32 = [&](std::size_t at) { return static_cast<uint32_t>(u16(at)) | static_cast<uint32_t>(u16(at + 2)) << 16; };
  if (data.size() < 12 || std::memcmp(data.data(), "RIFF", 4) != 0 || std::memcmp(data.data() + 8, "WAVE", 4) != 0)
    throw std::runtime_error(path + " is not a WAV file");
  std::size_t at = 12;
  bool format_ok = false;
  while (at + 8 <= data.size()) {
    const auto size = u32(at + 4);
    if (std::memcmp(data.data() + at, "fmt ", 4) == 0 && at + 8 + 16 <= data.size())
      format_ok = u16(at + 8) == 1 && u16(at + 10) == 1 && u32(at + 12) == 16000 && u16(at + 22) == 16;
    if (std::memcmp(data.data() + at, "data", 4) == 0) {
      if (!format_ok)
        throw std::runtime_error(path + " is not 16 kHz mono 16-bit PCM");
      const std::size_t count = std::min<std::size_t>(size, data.size() - at - 8) / 2;
      std::vector<float> samples(count);
      for (std::size_t i = 0; i < count; ++i)
        samples[i] = static_cast<float>(static_cast<int16_t>(u16(at + 8 + 2 * i))) / 32768.0f;
      return samples;
    }
    at += 8 + size + (size & 1);
  }
  throw std::runtime_error(path + " has no audio");
}

struct Command {
  nlohmann::json message;
};

class Server {
public:
  explicit Server(std::chrono::seconds idle_exit) : idle_exit_(idle_exit) {}

  int run() {
    emit({{"type", "hello"}, {"version", 1}, {"available", msime::voice::sherpa_runtime_available()}, {"error", msime::voice::sherpa_runtime_error()}});
#if !defined(_WIN32)
    if (::pipe(stop_pipe_) != 0)
      return 1;
#endif
    std::thread reader([this] { read_loop(); });
    work_loop();
#if !defined(_WIN32)
    // poll() makes the reader interruptible even when the parent keeps the
    // helper's stdin open while the idle timer expires. Join before this
    // object is destroyed; the old detached reader could outlive Server.
    // 被信号打断时重试：这一字节没写进去，reader 就永远不醒，下面的 join 会一直挂着。
    // 不能写成 `(void)::write(...)`，带 _FORTIFY_SOURCE 的 GCC 不认这种写法，在 -Werror 下直接编不过。
    const char stop = 1;
    while (::write(stop_pipe_[1], &stop, 1) < 0 && errno == EINTR) {
    }
#else
    // The Windows CRT has no pollable stdin descriptor. Closing the helper's
    // inherited input handle wakes getline so the reader can be joined before
    // this Server is destroyed.
    (void)_close(_fileno(stdin));
#endif
    reader.join();
#if !defined(_WIN32)
    ::close(stop_pipe_[0]);
    ::close(stop_pipe_[1]);
#endif
    return 0;
  }

private:
  void read_loop() {
#if !defined(_WIN32)
    std::string pending;
    std::array<char, 8192> buffer{};
    bool discarding_line = false;
    for (;;) {
      pollfd descriptors[] = {{stop_pipe_[0], POLLIN, 0}, {STDIN_FILENO, POLLIN, 0}};
      const int ready = ::poll(descriptors, 2, -1);
      if (ready < 0) {
        if (errno == EINTR) continue;
        break;
      }
      if (descriptors[0].revents & POLLIN) break;
      // macOS 的 poll() 不支持 /dev/null 这类设备文件，只回 POLLNVAL；不认它的话这里会立刻再 poll、空转到空闲退出（CI 里 `msime-voice-local < /dev/null` 因此每次卡满 600 秒）。交给下面的 read() 判断：/dev/null 读到 0 即 EOF，真正失效的描述符读出错，两种都结束循环。
      if (!(descriptors[1].revents & (POLLIN | POLLHUP | POLLERR | POLLNVAL))) continue;
      const auto count = ::read(STDIN_FILENO, buffer.data(), buffer.size());
      if (count == 0) break;
      if (count < 0) {
        if (errno == EINTR) continue;
        break;
      }
      if (discarding_line) {
        const auto *newline = static_cast<const char *>(std::memchr(buffer.data(), '\n', static_cast<size_t>(count)));
        if (!newline) continue;
        discarding_line = false;
        const auto remainder = static_cast<size_t>(count - (newline - buffer.data()) - 1);
        pending.assign(newline + 1, remainder);
      } else {
        pending.append(buffer.data(), static_cast<size_t>(count));
      }
      for (;;) {
        const auto newline = pending.find('\n');
        if (newline == std::string::npos) {
          if (!discarding_line && pending.size() > kMaxRequestLineBytes) {
            emit({{"type", "error"}, {"message", "request too large"}});
            pending.clear();
            discarding_line = true;
          }
          break;
        }
        auto line = pending.substr(0, newline);
        pending.erase(0, newline + 1);
        if (discarding_line) {
          discarding_line = false;
          continue;
        }
        handle_line(std::move(line));
      }
    }
#else
    std::string line;
    line.reserve(kMaxRequestLineBytes);
    bool discarding_line = false;
    char character = '\0';
    while (std::cin.get(character)) {
      if (character == '\n') {
        if (discarding_line) {
          emit({{"type", "error"}, {"message", "request too large"}});
          discarding_line = false;
          line.clear();
        } else {
          handle_line(std::move(line));
          line.clear();
        }
      } else if (!discarding_line) {
        if (line.size() == kMaxRequestLineBytes) {
          discarding_line = true;
          line.clear();
        } else {
          line.push_back(character);
        }
      }
    }
#endif
    std::lock_guard<std::mutex> lock(mutex_);
    closed_ = true;
    ready_.notify_one();
  }

  void handle_line(std::string line) {
    if (line.empty()) return;
    if (line.size() > kMaxRequestLineBytes) {
      emit({{"type", "error"}, {"message", "request too large"}});
      return;
    }
    nlohmann::json message;
    try {
      message = nlohmann::json::parse(line);
    } catch (const nlohmann::json::exception &) {
      emit({{"type", "error"}, {"message", "malformed request"}});
      return;
    }
    // `json::value` throws when an object field has the wrong type. This is a
    // protocol error from the caller, not a reason to terminate the helper's
    // reader thread and take local dictation down with it.
    if (!message.is_object() ||
        (message.contains("op") && !message.at("op").is_string())) {
      emit({{"type", "error"}, {"message", "malformed request"}});
      return;
    }
    // Cancellation takes effect in the middle of a decode, so it cannot wait its turn in the queue.
    if (message.value("op", std::string()) == "cancel") {
      std::lock_guard<std::mutex> lock(mutex_);
      if (cancelled_) cancelled_->store(true);
      if (queue_overflowed_) {
        overflow_cancelled_ = true;
        ready_.notify_one();
        return;
      }
    }
    std::lock_guard<std::mutex> lock(mutex_);
    if (queue_overflowed_) {
      // The worker will terminate the session after the command it is
      // currently decoding. Do not let a producer refill the queue while it
      // is unwinding that failure; cancel remains effective through the token
      // set above even though its response is no longer queued.
      return;
    }
    if (message.value("op", std::string()) == "start")
      queued_session_id_ = message.value("id", nlohmann::json());
    if (!queue_.try_push({std::move(message)}, line.size())) {
      queue_overflowed_ = true;
      overflow_id_ = !session_id_.is_null() ? session_id_ : queued_session_id_;
      if (cancelled_) cancelled_->store(true);
      ready_.notify_one();
      return;
    }
    ready_.notify_one();
  }

  void work_loop() {
    // Release a loaded model after this long without a session; the process itself exits after idle_exit_.
    // idle_exit_ of 0 disables the exit only; the 120 s model release still applies.
    const std::chrono::steady_clock::duration release_after =
        idle_exit_.count() > 0 ? std::min<std::chrono::steady_clock::duration>(idle_exit_, std::chrono::seconds(120))
                               : std::chrono::steady_clock::duration(std::chrono::seconds(120));
    auto last_activity = std::chrono::steady_clock::now();
    for (;;) {
      std::unique_lock<std::mutex> lock(mutex_);
      ready_.wait_for(lock, std::chrono::seconds(5), [this] { return closed_ || queue_overflowed_ || !queue_.empty(); });
      if (queue_overflowed_) {
        const auto error_id = overflow_id_;
        const bool cancelled = overflow_cancelled_;
        queue_.clear();
        queue_overflowed_ = false;
        overflow_cancelled_ = false;
        overflow_id_ = nlohmann::json();
        queued_session_id_ = nlohmann::json();
        session_.reset();
        session_id_ = nlohmann::json();
        cancelled_.reset();
        lock.unlock();
        if (cancelled && !error_id.is_null())
          emit({{"type", "cancelled"}, {"id", error_id}});
        else if (error_id.is_null())
          emit({{"type", "error"}, {"message", "request queue full"}});
        else
          emit({{"type", "error"}, {"id", error_id}, {"message", "request queue full"}});
        continue;
      }
      if (queue_.empty()) {
        if (closed_)
          return;
        lock.unlock();
        const auto idle = std::chrono::steady_clock::now() - last_activity;
        if (!session_) {
          msime::voice::release_idle_local_models(release_after);
          if (idle_exit_.count() > 0 && idle >= idle_exit_)
            return;
        }
        continue;
      }
      auto command = queue_.pop();
      lock.unlock();
      last_activity = std::chrono::steady_clock::now();
      handle(command->message);
    }
  }

  void handle(const nlohmann::json &message) {
    const auto op = message.value("op", std::string());
    const auto id = message.value("id", nlohmann::json());
    try {
      if (op == "start") {
        LocalAsrOptions options;
        options.model_dir = message.at("model").get<std::string>();
        options.language = message.value("language", std::string());
        options.threads = message.value("threads", 0);
        if (message.contains("hotwords"))
          options.hotwords = message.at("hotwords").get<std::vector<std::string>>();
        auto cancelled = std::make_shared<std::atomic_bool>(false);
        {
          std::lock_guard<std::mutex> lock(mutex_);
          cancelled_ = cancelled;
        }
        session_.reset();
        session_ = std::make_unique<LocalAsrSession>(
            options, [id](const std::string &text) { emit({{"type", "partial"}, {"id", id}, {"text", text}}); }, cancelled);
        {
          std::lock_guard<std::mutex> lock(mutex_);
          session_id_ = id;
          if (queued_session_id_ == id) queued_session_id_ = nlohmann::json();
        }
        if (!queue_overflowed()) emit({{"type", "started"}, {"id", id}});
      } else if (op == "audio") {
        if (!session_)
          return;
        const auto samples = decode_pcm16(message.at("pcm16").get<std::string>());
        if (!samples)
          throw std::runtime_error("audio is not base64 16-bit PCM");
        session_->accept(samples->data(), samples->size());
      } else if (op == "finish") {
        if (!session_)
          return;
        auto session = std::move(session_);
        nlohmann::json id;
        {
          std::lock_guard<std::mutex> lock(mutex_);
          id = session_id_;
        }
        const auto text = session->finish();
        if (!queue_overflowed()) emit({{"type", "final"}, {"id", id}, {"text", text}});
        std::lock_guard<std::mutex> lock(mutex_);
        session_id_ = nlohmann::json();
        cancelled_.reset();
      } else if (op == "cancel") {
        if (session_) {
          nlohmann::json id;
          {
            std::lock_guard<std::mutex> lock(mutex_);
            id = session_id_;
          }
          session_.reset();
          {
            std::lock_guard<std::mutex> lock(mutex_);
            session_id_ = nlohmann::json();
            cancelled_.reset();
          }
          if (!queue_overflowed()) emit({{"type", "cancelled"}, {"id", id}});
        }
      } else if (op == "release") {
        msime::voice::release_local_models();
      } else if (op == "ping") {
        emit({{"type", "pong"}, {"id", id}, {"available", msime::voice::sherpa_runtime_available()}});
      } else {
        throw std::runtime_error("unknown op");
      }
    } catch (const std::exception &error) {
      std::shared_ptr<std::atomic_bool> cancelled_token;
      {
        std::lock_guard<std::mutex> lock(mutex_);
        cancelled_token = cancelled_;
      }
      const bool cancelled = cancelled_token && cancelled_token->load();
      nlohmann::json error_id;
      {
        std::lock_guard<std::mutex> lock(mutex_);
        error_id = op == "start" ? id : session_id_;
      }
      session_.reset();
      {
        std::lock_guard<std::mutex> lock(mutex_);
        session_id_ = nlohmann::json();
        cancelled_.reset();
      }
      if (!queue_overflowed())
        emit({{"type", cancelled ? "cancelled" : "error"}, {"id", error_id}, {"message", error.what()}});
    }
  }

  bool queue_overflowed() {
    std::lock_guard<std::mutex> lock(mutex_);
    return queue_overflowed_;
  }

  std::chrono::seconds idle_exit_;
  std::mutex mutex_;
  std::condition_variable ready_;
  msime::voice::BoundedCommandQueue<Command> queue_{kMaxQueuedRequestBytes};
  bool queue_overflowed_ = false;
  bool overflow_cancelled_ = false;
  nlohmann::json overflow_id_;
  nlohmann::json queued_session_id_;
  bool closed_ = false;
  std::shared_ptr<std::atomic_bool> cancelled_;
  std::unique_ptr<LocalAsrSession> session_;
  nlohmann::json session_id_;
#if !defined(_WIN32)
  int stop_pipe_[2]{-1, -1};
#endif
};

// Whole non-negative decimal seconds, nothing else.
bool parse_seconds(const std::string &text, std::chrono::seconds &out) {
  if (text.empty() || text[0] < '0' || text[0] > '9')
    return false;
  errno = 0;
  char *end = nullptr;
  const long parsed = std::strtol(text.c_str(), &end, 10);
  if (errno != 0 || end != text.c_str() + text.size() || parsed > 100000000)
    return false;
  out = std::chrono::seconds(parsed);
  return true;
}

} // namespace

int main(int argc, char **argv) {
#if defined(_WIN32)
  _setmode(_fileno(stdout), _O_BINARY);
  _setmode(_fileno(stdin), _O_BINARY);
#endif
  std::string model;
  std::string wav;
  std::string language;
  std::vector<std::string> hotwords;
  std::chrono::seconds idle_exit(600);
  for (int i = 1; i < argc; ++i) {
    const std::string argument = argv[i];
    auto value = [&]() -> std::string {
      if (i + 1 >= argc) {
        std::fprintf(stderr, "%s needs a value\n", argument.c_str());
        std::exit(2);
      }
      return argv[++i];
    };
    if (argument == "--model")
      model = value();
    else if (argument == "--wav")
      wav = value();
    else if (argument == "--language")
      language = value();
    else if (argument == "--hotword")
      hotwords.push_back(value());
    else if (argument == "--runtime")
      msime::voice::set_sherpa_library_path(value());
    else if (argument == "--idle-exit" && parse_seconds(value(), idle_exit))
      ; // a malformed or negative value falls through to usage
    else {
      std::fprintf(stderr, "usage: msime-voice-local [--runtime <library>] [--idle-exit <seconds>] [--model <dir> --wav <file> [--language <tag>] [--hotword <word>]...]\n");
      return 2;
    }
  }
  if (!wav.empty()) {
    try {
      LocalAsrOptions options;
      options.model_dir = model;
      options.language = language;
      options.hotwords = hotwords;
      const auto text = msime::voice::recognize_local_model(read_wav(wav), options, nullptr);
      std::fwrite(text.data(), 1, text.size(), stdout);
      std::fputc('\n', stdout);
      return 0;
    } catch (const std::exception &error) {
      std::fprintf(stderr, "%s\n", error.what());
      return 1;
    }
  }
  return Server(idle_exit).run();
}
