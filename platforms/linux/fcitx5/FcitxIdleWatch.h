#pragma once

#include <cstdint>
#include <filesystem>
#include <functional>
#include <initializer_list>
#include <map>
#include <memory>
#include <set>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

namespace fcitx {
class EventLoop;
struct EventSourceIO;
struct EventSourceTime;
} // namespace fcitx

namespace msime::fcitx_host {

// 插件级外部文件与目录监听配置，不依赖 JSON 库以便独立编译行为测试。
struct FcitxWatchConfig {
  // 精确监听的外部文件路径（如 `runtime-options.json`、`classicui.conf`、词库维护租约、`clipboard_history.json`）。
  std::vector<std::filesystem::path> files;
  // 偏好目录（监听除 `diagnostic.log`、`*.lock` 与私有文件原子写入的临时文件 `.msime-private-*` 之外的写入、替换和删除；`typing-statistics.json` 单独走统计开关回调，见 `FcitxIdleWatch::setStatisticsCallback`）。
  std::vector<std::filesystem::path> preference_dirs;
  // 运行时套接字目录（仅监听除 `panel-input.sock` 之外的 `*.sock` 文件变化）。
  std::vector<std::filesystem::path> socket_dirs;

  bool empty() const noexcept {
    return files.empty() && preference_dirs.empty() && socket_dirs.empty();
  }

  bool operator==(const FcitxWatchConfig &other) const noexcept {
    return files == other.files && preference_dirs == other.preference_dirs &&
           socket_dirs == other.socket_dirs;
  }
};

// 解析当前环境变量下的用户配置根目录（优先 `XDG_CONFIG_HOME`，回退 `HOME/.config`）。
std::filesystem::path resolveFcitxConfigHome();

// 组装插件级基础监听目标（`runtime-options.json`、`classicui.conf` 与 `$XDG_RUNTIME_DIR/<client_dir>` 套接字目录）。
FcitxWatchConfig buildBaseFcitxWatchConfig(
    const std::filesystem::path &initial_config_home,
    const char *client_directory,
    const char *system_options_path);

// 追加单个会话上下文持有的偏好目录、词库维护租约、剪贴板历史与显式套接字路径。
void appendContextWatchPaths(
    FcitxWatchConfig &watch,
    const std::string &options_path,
    const std::string &dictionary_user_data,
    std::string_view quiesce_lease_name,
    const std::string &clipboard_path,
    std::initializer_list<const std::string *> sockets);

// 基于单实例 `inotify_init1(IN_NONBLOCK | IN_CLOEXEC)` 与 50ms 一次性防抖的空闲零轮询监听器；
// 仅在内核无法分配 `inotify` 实例（`inotify_init1 < 0`）时回退到低频定时器。
class FcitxIdleWatch {
public:
  using Callback = std::function<void()>;
  using InotifyInitFn = int (*)(int);

  FcitxIdleWatch(fcitx::EventLoop &loop, Callback callback,
                 std::uint64_t debounce_us = 50'000,
                 std::uint64_t fallback_interval_us = 1'000'000,
                 InotifyInitFn init_fn = nullptr);
  ~FcitxIdleWatch();
  FcitxIdleWatch(const FcitxIdleWatch &) = delete;
  FcitxIdleWatch &operator=(const FcitxIdleWatch &) = delete;

  void configure(FcitxWatchConfig config);
  // 偏好目录里只有 `typing-statistics.json` 变了时改调这个回调，不触发整套偏好重载：插件自己每次刷新打字统计都会改写它，而它同时载着统计总开关，设置应用关掉统计时也只改这一个文件，所以不能直接忽略。没设时按普通外部变化处理。
  void setStatisticsCallback(Callback callback) { statistics_callback_ = std::move(callback); }
  void setActive(bool active);
  bool isActive() const noexcept { return active_; }
  bool usingFallback() const noexcept { return inotify_fd_ < 0; }
  bool hasArmedTimer() const noexcept;

private:
  struct WatchedDirSpec {
    std::set<std::string> exact_files;
    std::set<std::string> ancestor_steps;
    bool preference_dir = false;
    bool socket_dir = false;

    bool operator==(const WatchedDirSpec &other) const noexcept {
      return exact_files == other.exact_files &&
             ancestor_steps == other.ancestor_steps &&
             preference_dir == other.preference_dir &&
             socket_dir == other.socket_dir;
    }
  };

  struct WatchedDirEntry {
    std::filesystem::path directory;
    WatchedDirSpec spec;
  };

  static void normalizePaths(std::vector<std::filesystem::path> &paths);
  void rebuildWatches();
  void handleInotifyReadable();
  void armDebounce();
  void syncFallbackTimer();

  fcitx::EventLoop &loop_;
  Callback callback_;
  Callback statistics_callback_;
  bool pending_full_ = false;
  bool pending_statistics_ = false;
  std::uint64_t debounce_us_;
  std::uint64_t fallback_interval_us_;
  int inotify_fd_ = -1;
  bool active_ = true;
  FcitxWatchConfig config_;
  std::map<std::filesystem::path, int> watched_by_path_;
  std::unordered_map<int, WatchedDirEntry> watched_by_wd_;
  std::unique_ptr<fcitx::EventSourceIO> io_event_;
  std::unique_ptr<fcitx::EventSourceTime> debounce_timer_;
  std::unique_ptr<fcitx::EventSourceTime> fallback_timer_;
};

} // namespace msime::fcitx_host
