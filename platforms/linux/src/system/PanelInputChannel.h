#pragma once

#include <cctype>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <fcntl.h>
#include <nlohmann/json.hpp>
#include <optional>
#include <poll.h>
#include <string>
#include <system_error>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>
#include <utility>
#include <vector>

namespace msime::linux_host {

// The shared desktop panels (screen keyboard, handwriting, emoji, clipboard, voice) are ordinary Tauri windows with no input context of their own. Windows hands their output to SendInput, which passes through the active IME before it reaches the editor; the Linux equivalent that works on every session type is the input method itself, which already owns a connection to the focused editor. The panel process sends one JSON line over a user-private socket and the host commits the text, or runs the key through its own key handling first (see deliver_panel_key_stroke), into the focused context. xdotool, wtype and ydotool stay as the fallback for sessions where the MSIME host is not the active one.
//
// Requests:
//   {"op":"generation"}
//   {"op":"text","text":"...","after_generation":N}
//   {"op":"key","key":"<X keysym name>","keycode":<evdev>,"shift":b,"control":b,"alt":b,"super":b,"after_generation":N}
// Replies: {"ok":true[,"generation":N]} or {"ok":false,"error":"no_focus|restricted|invalid"}.
//
// after_generation exists because a panel the user just clicked may hold the focus itself. The panel reads the focus generation, hides itself, and asks for delivery only to a context that was focused after that generation, so the text cannot land in the panel's own web view.
struct PanelInputRequest {
  enum class Kind { Generation, Text, Key };
  Kind kind = Kind::Generation;
  std::string text;
  std::string key;
  uint32_t keycode = 0;
  bool shift = false;
  bool control = false;
  bool alt = false;
  bool super = false;
  std::optional<uint64_t> after_generation;
};

inline constexpr size_t kPanelInputLineLimit = 16384;
inline constexpr size_t kPanelInputTextLimit = 4096;
inline constexpr size_t kPanelInputPendingLimit = 128;
inline constexpr int64_t kPanelInputWaitUs = 700000;

inline std::optional<PanelInputRequest> parse_panel_input_request(const std::string &line) {
  if (line.size() > kPanelInputLineLimit) return std::nullopt;
  const auto json = nlohmann::json::parse(line, nullptr, false);
  if (!json.is_object()) return std::nullopt;
  const auto op = json.find("op");
  if (op == json.end() || !op->is_string()) return std::nullopt;
  PanelInputRequest request;
  if (const auto after = json.find("after_generation"); after != json.end()) {
    if (!after->is_number_unsigned()) return std::nullopt;
    request.after_generation = after->get<uint64_t>();
  }
  const auto flag = [&](const char *name, bool &value) {
    const auto found = json.find(name);
    if (found == json.end()) return true;
    if (!found->is_boolean()) return false;
    value = found->get<bool>();
    return true;
  };
  const auto kind = op->get<std::string>();
  if (kind == "generation") {
    request.kind = PanelInputRequest::Kind::Generation;
    return request;
  }
  if (kind == "text") {
    const auto text = json.find("text");
    if (text == json.end() || !text->is_string()) return std::nullopt;
    request.kind = PanelInputRequest::Kind::Text;
    request.text = text->get<std::string>();
    // Line breaks and tabs would reach a terminal or a form as Enter and Tab; the panel sends those through the clipboard instead, so the IME route only carries single-line text.
    if (request.text.empty() || request.text.size() > kPanelInputTextLimit) return std::nullopt;
    for (const unsigned char byte : request.text)
      if (byte < 0x20 || byte == 0x7f) return std::nullopt;
    return request;
  }
  if (kind == "key") {
    const auto key = json.find("key");
    if (key == json.end() || !key->is_string()) return std::nullopt;
    request.kind = PanelInputRequest::Kind::Key;
    request.key = key->get<std::string>();
    if (request.key.empty() || request.key.size() > 64) return std::nullopt;
    for (const unsigned char byte : request.key)
      if (!std::isalnum(byte) && byte != '_') return std::nullopt;
    if (const auto keycode = json.find("keycode"); keycode != json.end()) {
      if (!keycode->is_number_unsigned() || keycode->get<uint64_t>() > 247) return std::nullopt;
      request.keycode = keycode->get<uint32_t>();
    }
    if (!flag("shift", request.shift) || !flag("control", request.control) ||
        !flag("alt", request.alt) || !flag("super", request.super))
      return std::nullopt;
    return request;
  }
  return std::nullopt;
}

inline std::string panel_input_ok_reply() { return R"({"ok":true})"; }

inline std::string panel_input_generation_reply(uint64_t generation) {
  return nlohmann::json{{"ok", true}, {"generation", generation}}.dump();
}

inline std::string panel_input_error_reply(const char *error) {
  return nlohmann::json{{"ok", false}, {"error", error}}.dump();
}

inline int64_t panel_input_monotonic_us() {
  timespec now{};
  clock_gettime(CLOCK_MONOTONIC, &now);
  return static_cast<int64_t>(now.tv_sec) * 1000000 + now.tv_nsec / 1000;
}

enum class PanelInputDelivery { Delivered, Restricted, Invalid, NoFocus };

struct PanelInputFocus {
  bool focused = false;
  uint64_t generation = 0;
};

// A screen keyboard key takes the path SendInput gives it on Windows: through the input method first, so letters build a composition and digits, Space and BackSpace act on an open one, and on to the editor only when the input method leaves the press alone. The release always reaches the input method too, since hosts track state across a stroke (a BackSpace hold, a consumed shortcut stroke). When the press goes to the editor its release follows, whatever the input method did with the release, so the editor never sees half a stroke.
//
// Process takes whether the event is the release and returns whether the input method consumed it; Forward takes whether the event is the release and sends it on to the editor.
template <class Process, class Forward>
void deliver_panel_key_stroke(Process process, Forward forward) {
  const bool consumed = process(false);
  if (!consumed) forward(false);
  process(true);
  if (!consumed) forward(true);
}

// Holds requests until a context can take them. A request that cannot be delivered within kPanelInputWaitUs is answered no_focus and dropped, so a focus that arrives later can never type it a second time after the panel has already fallen back to another route. The pending count is capped so a same-user client cannot retain an arbitrary number of open request connections while focus is unavailable.
class PanelInputBroker {
public:
  PanelInputBroker() { pending_.reserve(kPanelInputPendingLimit); }
  PanelInputBroker(const PanelInputBroker &) = delete;
  PanelInputBroker &operator=(const PanelInputBroker &) = delete;
  ~PanelInputBroker() {
    for (const auto &entry : pending_) ::close(entry.fd);
  }

  bool submit(int fd, PanelInputRequest request, int64_t now_us) {
    if (pending_.size() >= kPanelInputPendingLimit) return false;
    pending_.push_back({fd, std::move(request), now_us + kPanelInputWaitUs});
    return true;
  }

  bool empty() const { return pending_.empty(); }

  // Focus returns PanelInputFocus, Deliver takes a request and returns PanelInputDelivery, and Reply takes the connection and the reply line and owns closing it. Requests are served in order: one still waiting for focus holds back those behind it, which may only expire.
  template <class Focus, class Deliver, class Reply>
  void pump(int64_t now_us, Focus focus, Deliver deliver, Reply reply) {
    bool held = false;
    for (auto entry = pending_.begin(); entry != pending_.end();) {
      if (entry->request.kind == PanelInputRequest::Kind::Generation) {
        reply(entry->fd, panel_input_generation_reply(focus().generation));
        entry = pending_.erase(entry);
        continue;
      }
      if (!held) {
        const auto current = focus();
        const bool ready = current.focused && (!entry->request.after_generation ||
                                               current.generation > *entry->request.after_generation);
        const auto outcome = ready ? deliver(entry->request) : PanelInputDelivery::NoFocus;
        if (outcome != PanelInputDelivery::NoFocus) {
          reply(entry->fd, outcome == PanelInputDelivery::Delivered ? panel_input_ok_reply()
                           : outcome == PanelInputDelivery::Restricted
                               ? panel_input_error_reply("restricted")
                               : panel_input_error_reply("invalid"));
          entry = pending_.erase(entry);
          continue;
        }
      }
      if (now_us >= entry->deadline_us) {
        reply(entry->fd, panel_input_error_reply("no_focus"));
        entry = pending_.erase(entry);
        continue;
      }
      held = true;
      ++entry;
    }
  }

private:
  struct Entry {
    int fd;
    PanelInputRequest request;
    int64_t deadline_us;
  };
  std::vector<Entry> pending_;
};

inline std::string panel_input_socket_path() {
  const char *runtime = std::getenv("XDG_RUNTIME_DIR");
  if (!runtime || runtime[0] != '/') return {};
  return std::string(runtime) + "/msime-client/panel-input.sock";
}

// 逐组件检查 socket 目录，避免 mkdir 沿着中间符号链接在外部创建目录。
inline bool panel_input_directory_is_safe(const std::filesystem::path &directory) {
  if (!directory.is_absolute()) return false;
  std::filesystem::path current = directory.root_path();
  bool saw_prefix_alias = false;
  bool saw_real_component = false;
  std::error_code error;
  for (const auto &component : directory) {
    if (component == directory.root_name() || component == directory.root_directory()) continue;
    current /= component;
    const auto status = std::filesystem::symlink_status(current, error);
    if (!error) {
      if (std::filesystem::is_symlink(status)) {
        const bool system_alias = !saw_real_component && !saw_prefix_alias &&
                                  (component == "tmp" || component == "var");
        if (!system_alias) return false;
        saw_prefix_alias = true;
        continue;
      }
      if (!std::filesystem::is_directory(status)) return false;
      saw_real_component = true;
      continue;
    }
    if (error != std::errc::no_such_file_or_directory) return false;
    error.clear();
  }
  return true;
}

// The listening socket. IBus and Fcitx5 may both be installed; whichever host binds first serves the panels, and the other leaves a live socket alone rather than stealing it.
class PanelInputSocket {
public:
  PanelInputSocket() = default;
  PanelInputSocket(const PanelInputSocket &) = delete;
  PanelInputSocket &operator=(const PanelInputSocket &) = delete;
  ~PanelInputSocket() { close(); }

  int fd() const { return fd_; }
  bool listening() const { return fd_ >= 0; }

  bool open(const std::string &path) {
    if (fd_ >= 0) return true;
    sockaddr_un address{};
    if (path.empty() || path.size() >= sizeof(address.sun_path)) return false;
    const auto slash = path.rfind('/');
    if (slash == std::string::npos || slash == 0) return false;
    const auto directory = path.substr(0, slash);
    if (!panel_input_directory_is_safe(std::filesystem::path(directory))) return false;
    if (::mkdir(directory.c_str(), 0700) != 0 && errno != EEXIST) return false;
    struct stat info {};
    // Other session services share this directory. It must belong to this user and admit no one else's writes; the socket itself is 0600 and every peer is checked below as well.
    if (::lstat(directory.c_str(), &info) != 0 || !S_ISDIR(info.st_mode) ||
        info.st_uid != ::getuid() || (info.st_mode & 022) != 0)
      return false;
    address.sun_family = AF_UNIX;
    std::memcpy(address.sun_path, path.c_str(), path.size() + 1);
    if (::lstat(path.c_str(), &info) == 0) {
      if (!S_ISSOCK(info.st_mode)) return false;
      const int probe = ::socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
      if (probe < 0) return false;
      const bool live =
          ::connect(probe, reinterpret_cast<const sockaddr *>(&address), sizeof(address)) == 0;
      ::close(probe);
      if (live) return false;
      ::unlink(path.c_str());
    }
    const int fd = ::socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
    if (fd < 0) return false;
    if (::bind(fd, reinterpret_cast<const sockaddr *>(&address), sizeof(address)) != 0 ||
        ::chmod(path.c_str(), 0600) != 0 || ::listen(fd, 8) != 0 ||
        ::lstat(path.c_str(), &info) != 0) {
      ::close(fd);
      return false;
    }
    fd_ = fd;
    path_ = path;
    inode_ = info.st_ino;
    return true;
  }

  // Accepts one connection and reads its request line. Returns the connection and the line, or nothing when no connection was waiting or the peer was refused; a refused peer is closed here.
  std::optional<std::pair<int, std::string>> accept_request() {
    if (fd_ < 0) return std::nullopt;
    const int connection = ::accept4(fd_, nullptr, nullptr, SOCK_CLOEXEC);
    if (connection < 0) return std::nullopt;
    ucred peer{};
    socklen_t size = sizeof(peer);
    if (::getsockopt(connection, SOL_SOCKET, SO_PEERCRED, &peer, &size) != 0 ||
        peer.uid != ::getuid()) {
      ::close(connection);
      return std::nullopt;
    }
    // The panel writes its line straight after connecting. Bound the wait so a stalled peer cannot hold the host's event loop.
    std::string line;
    const int64_t deadline = panel_input_monotonic_us() + 200000;
    char buffer[1024];
    while (true) {
      const int64_t remaining = deadline - panel_input_monotonic_us();
      pollfd ready{connection, POLLIN, 0};
      if (remaining <= 0 || ::poll(&ready, 1, static_cast<int>(remaining / 1000) + 1) <= 0) break;
      const auto count = ::recv(connection, buffer, sizeof(buffer), MSG_DONTWAIT);
      if (count <= 0) break;
      line.append(buffer, static_cast<size_t>(count));
      if (const auto end = line.find('\n'); end != std::string::npos) {
        line.resize(end);
        return std::make_pair(connection, std::move(line));
      }
      if (line.size() > kPanelInputLineLimit) break;
    }
    ::close(connection);
    return std::nullopt;
  }

  static void reply_and_close(int connection, const std::string &reply) {
    const std::string line = reply + "\n";
    ::send(connection, line.data(), line.size(), MSG_NOSIGNAL | MSG_DONTWAIT);
    ::close(connection);
  }

  void close() {
    if (fd_ < 0) return;
    ::close(fd_);
    fd_ = -1;
    // Only remove the path while it is still this socket; another host may have taken it over.
    struct stat info {};
    if (::lstat(path_.c_str(), &info) == 0 && info.st_ino == inode_) ::unlink(path_.c_str());
  }

private:
  int fd_ = -1;
  std::string path_;
  ino_t inode_ = 0;
};

}  // namespace msime::linux_host
