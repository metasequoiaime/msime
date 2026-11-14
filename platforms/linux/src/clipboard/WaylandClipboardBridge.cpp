#include "WaylandClipboardBridge.h"
#include "wlr-data-control-unstable-v1-client-protocol.h"
#include <wayland-client.h>
#include <nlohmann/json.hpp>
#include <algorithm>
#include <chrono>
#include <csignal>
#include <cstdio>
#include <fcntl.h>
#include <list>
#include <map>
#include <memory>
#include <poll.h>
#include <string>
#include <unistd.h>
#include <vector>

namespace {
using Json = nlohmann::json;
using Clock = std::chrono::steady_clock;
constexpr size_t kMaxBytes = 12000;

bool valid_text(const std::string &text) {
  if (text.find('\0') != std::string::npos || text.size() > kMaxBytes) return false;
  size_t units = 0;
  for (unsigned char ch : text) {
    if ((ch & 0xc0) != 0x80) units += ch >= 0xf0 ? 2 : 1;
  }
  if (units > 4000) return false;
  try { (void)Json(text).dump(); } catch (const Json::exception &) { return false; }
  return true;
}

class Bridge {
  struct Offer {
    zwlr_data_control_offer_v1 *object = nullptr;
    std::vector<std::string> types;
  };
  struct Source {
    Bridge *bridge;
    zwlr_data_control_source_v1 *object;
    std::string text, marker;
    uint64_t request;
    bool cancelled = false;
  };
  struct Sending {
    int fd;
    std::string text;
    size_t offset;
    Clock::time_point deadline;
  };
  wl_display *display_ = nullptr;
  wl_registry *registry_ = nullptr;
  wl_seat *seat_ = nullptr;
  zwlr_data_control_manager_v1 *manager_ = nullptr;
  zwlr_data_control_device_v1 *device_ = nullptr;
  std::map<zwlr_data_control_offer_v1 *, std::unique_ptr<Offer>> offers_;
  std::list<std::unique_ptr<Source>> sources_;
  std::list<Sending> sending_;
  uint64_t generation_ = 0;
  int read_fd_ = -1;
  std::string reading_, commands_;
  Clock::time_point deadline_;
  bool running_ = true, pending_ = true;
  Json text_ = nullptr;
  bool remote_ = false;

  void emit(Json extra = Json::object()) {
    extra["generation"] = generation_;
    extra["text"] = text_;
    extra["pending"] = pending_;
    extra["remote"] = remote_;
    const auto line = extra.dump() + "\n";
    if (std::fwrite(line.data(), 1, line.size(), stdout) != line.size() ||
        std::fflush(stdout) != 0) running_ = false;
  }

  void finish_read() {
    if (read_fd_ >= 0) close(read_fd_);
    read_fd_ = -1;
    reading_.clear();
    pending_ = false;
  }

  static void global(void *data, wl_registry *registry, uint32_t name,
                     const char *interface, uint32_t version) {
    auto &self = *static_cast<Bridge *>(data);
    if (std::string(interface) == zwlr_data_control_manager_v1_interface.name && !self.manager_) {
      self.manager_ = static_cast<zwlr_data_control_manager_v1 *>(wl_registry_bind(
          registry, name, &zwlr_data_control_manager_v1_interface, std::min(version, 2u)));
    } else if (std::string(interface) == wl_seat_interface.name && !self.seat_) {
      self.seat_ = static_cast<wl_seat *>(wl_registry_bind(registry, name, &wl_seat_interface, 1));
    }
  }
  static void removed(void *, wl_registry *, uint32_t) {}
  static void offer_type(void *data, zwlr_data_control_offer_v1 *, const char *type) {
    static_cast<Offer *>(data)->types.emplace_back(type);
  }
  static void offer(void *data, zwlr_data_control_device_v1 *, zwlr_data_control_offer_v1 *object) {
    auto &self = *static_cast<Bridge *>(data);
    auto value = std::make_unique<Offer>();
    value->object = object;
    static const zwlr_data_control_offer_v1_listener listener{offer_type};
    zwlr_data_control_offer_v1_add_listener(object, &listener, value.get());
    self.offers_.emplace(object, std::move(value));
  }
  void destroy_offer(zwlr_data_control_offer_v1 *object) {
    if (!object) return;
    zwlr_data_control_offer_v1_destroy(object);
    offers_.erase(object);
  }
  static void selection(void *data, zwlr_data_control_device_v1 *, zwlr_data_control_offer_v1 *object) {
    auto &self = *static_cast<Bridge *>(data);
    self.finish_read();
    ++self.generation_;
    self.text_ = nullptr;
    self.remote_ = false;
    auto found = self.offers_.find(object);
    if (!object) {
      self.text_ = "";
      self.emit();
      return;
    }
    if (found == self.offers_.end()) { self.emit(); return; }
    const auto types = found->second->types;
    auto has = [&types](const std::string &type) {
      return std::find(types.begin(), types.end(), type) != types.end();
    };
    // 由本连接持有的来源只通过唯一 MIME 标记确认，不把同文本的用户复制当回声。
    for (const auto &source : self.sources_) {
      if (!source->cancelled && has(source->marker)) {
        self.text_ = source->text;
        self.remote_ = true;
        self.emit({{"request", source->request}, {"written", true}});
        self.destroy_offer(object);
        return;
      }
    }
    const bool protected_text = has("x-kde-passwordManagerHint") ||
        has("application/x-kde-passwordManagerHint") || has("application/x-keepassxc");
    std::string mime;
    if (!protected_text) {
      for (const char *type : {"text/plain;charset=utf-8", "text/plain", "UTF8_STRING"}) {
        if (has(type)) { mime = type; break; }
      }
    }
    if (!mime.empty()) {
      int fds[2];
      if (pipe2(fds, O_CLOEXEC) == 0) {
        fcntl(fds[0], F_SETFL, O_NONBLOCK);
        self.read_fd_ = fds[0];
        self.pending_ = true;
        self.deadline_ = Clock::now() + std::chrono::seconds(1);
        zwlr_data_control_offer_v1_receive(object, mime.c_str(), fds[1]);
        close(fds[1]);
      }
    }
    // 包括图片、敏感标记与正在读取的选区，先撤销旧文本快照。
    self.emit();
    self.destroy_offer(object);
  }
  static void primary(void *data, zwlr_data_control_device_v1 *, zwlr_data_control_offer_v1 *object) {
    static_cast<Bridge *>(data)->destroy_offer(object);
  }
  static void finished(void *data, zwlr_data_control_device_v1 *) {
    static_cast<Bridge *>(data)->running_ = false;
  }
  static void send(void *data, zwlr_data_control_source_v1 *, const char *, int32_t fd) {
    auto &source = *static_cast<Source *>(data);
    // 正文有界；独立请求者不读管道时不能阻塞选区变化和账号失效处理。
    fcntl(fd, F_SETFL, O_NONBLOCK);
    source.bridge->sending_.push_back({fd, source.text, 0, Clock::now() + std::chrono::seconds(1)});
  }
  void send_pending() {
    for (auto entry = sending_.begin(); entry != sending_.end();) {
      const auto count = write(entry->fd, entry->text.data() + entry->offset,
                               entry->text.size() - entry->offset);
      if (count > 0) entry->offset += static_cast<size_t>(count);
      if (entry->offset == entry->text.size() || Clock::now() >= entry->deadline ||
          (count < 0 && errno != EAGAIN && errno != EINTR)) {
        close(entry->fd);
        entry = sending_.erase(entry);
      } else {
        ++entry;
      }
    }
  }
  static void cancelled(void *data, zwlr_data_control_source_v1 *) {
    auto &source = *static_cast<Source *>(data);
    source.cancelled = true;
    zwlr_data_control_source_v1_destroy(source.object);
  }
  void receive(const Json &command) {
    const auto request = command.at("request").get<uint64_t>();
    const auto expected = command.at("generation").get<uint64_t>();
    const auto text = command.at("text").get<std::string>();
    // 先处理合成器已排队的变化，再比较选区代次；不依赖 Python 缓存的正文。
    if (wl_display_roundtrip(display_) < 0) { running_ = false; return; }
    if (!running_ || pending_ || expected != generation_ || text_.is_null() ||
        text.empty() || !valid_text(text)) {
      emit({{"request", request}, {"written", false}});
      return;
    }
    auto source = std::make_unique<Source>();
    source->bridge = this;
    source->object = zwlr_data_control_manager_v1_create_data_source(manager_);
    source->text = text;
    source->request = request;
    source->marker = "application/x-msime-cloud-clipboard-" + std::to_string(getpid()) + "-" +
        std::to_string(Clock::now().time_since_epoch().count());
    static const zwlr_data_control_source_v1_listener listener{send, cancelled};
    zwlr_data_control_source_v1_add_listener(source->object, &listener, source.get());
    zwlr_data_control_source_v1_offer(source->object, "text/plain;charset=utf-8");
    zwlr_data_control_source_v1_offer(source->object, "text/plain");
    zwlr_data_control_source_v1_offer(source->object, source->marker.c_str());
    zwlr_data_control_device_v1_set_selection(device_, source->object);
    sources_.push_back(std::move(source));
  }
  void read_selection() {
    char buffer[4096];
    const auto size = read(read_fd_, buffer, sizeof(buffer));
    if (size > 0) {
      reading_.append(buffer, static_cast<size_t>(size));
      if (reading_.size() > kMaxBytes) { finish_read(); emit(); }
    } else if (size == 0) {
      if (valid_text(reading_)) text_ = reading_;
      finish_read();
      emit();
    } else if (errno != EAGAIN && errno != EINTR) {
      finish_read();
      emit();
    }
  }

public:
  ~Bridge() {
    finish_read();
    for (const auto &entry : sending_) close(entry.fd);
    for (const auto &source : sources_) {
      if (!source->cancelled) zwlr_data_control_source_v1_destroy(source->object);
    }
    for (const auto &entry : offers_) zwlr_data_control_offer_v1_destroy(entry.first);
    if (device_) zwlr_data_control_device_v1_destroy(device_);
    if (manager_) zwlr_data_control_manager_v1_destroy(manager_);
    if (seat_) wl_seat_destroy(seat_);
    if (registry_) wl_registry_destroy(registry_);
    if (display_) wl_display_disconnect(display_);
  }
  int run() {
    signal(SIGPIPE, SIG_IGN);
    display_ = wl_display_connect(nullptr);
    if (!display_) return 1;
    registry_ = wl_display_get_registry(display_);
    static const wl_registry_listener globals{global, removed};
    wl_registry_add_listener(registry_, &globals, this);
    if (wl_display_roundtrip(display_) < 0 || !seat_ || !manager_) return 2;
    device_ = zwlr_data_control_manager_v1_get_data_device(manager_, seat_);
    static const zwlr_data_control_device_v1_listener device_listener{offer, selection, finished, primary};
    zwlr_data_control_device_v1_add_listener(device_, &device_listener, this);
    fcntl(STDIN_FILENO, F_SETFL, O_NONBLOCK);
    while (running_) {
      if (wl_display_dispatch_pending(display_) < 0 || wl_display_flush(display_) < 0) break;
      send_pending();
      pollfd fds[]{{wl_display_get_fd(display_), POLLIN, 0}, {STDIN_FILENO, POLLIN, 0},
                   {read_fd_, POLLIN, 0}};
      if (poll(fds, 3, 100) < 0) { if (errno == EINTR) continue; break; }
      if (fds[0].revents && wl_display_dispatch(display_) < 0) break;
      // dispatch 可能已换选区，旧描述符的就绪状态不能用于新管道。
      if (read_fd_ >= 0 && fds[2].fd == read_fd_ && fds[2].revents) read_selection();
      if (read_fd_ >= 0 && Clock::now() >= deadline_) { finish_read(); emit(); }
      if (fds[1].revents) {
        char buffer[4096];
        const auto count = read(STDIN_FILENO, buffer, sizeof(buffer));
        if (count <= 0) break;
        commands_.append(buffer, static_cast<size_t>(count));
        if (commands_.size() > 65536) break;
        size_t end;
        while ((end = commands_.find('\n')) != std::string::npos) {
          const auto line = commands_.substr(0, end);
          commands_.erase(0, end + 1);
          try { receive(Json::parse(line)); } catch (const Json::exception &) {}
        }
      }
      sources_.remove_if([](const auto &source) { return source->cancelled; });
    }
    return 0;
  }
};
} // namespace

int msime_run_wayland_clipboard_bridge() { return Bridge().run(); }
