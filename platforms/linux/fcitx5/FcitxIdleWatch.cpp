#include "FcitxIdleWatch.h"

#include <fcitx-utils/event.h>

#include <algorithm>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <string_view>
#include <sys/inotify.h>
#include <system_error>
#include <unistd.h>
#include <utility>

namespace msime::fcitx_host {

std::filesystem::path resolveFcitxConfigHome() {
  if (const auto *xdg = std::getenv("XDG_CONFIG_HOME"); xdg && *xdg) {
    std::filesystem::path path(xdg);
    if (path.is_absolute()) return path;
  }
  if (const auto *home = std::getenv("HOME"); home && *home) {
    std::filesystem::path path = std::filesystem::path(home) / ".config";
    if (path.is_absolute()) return path;
  }
  return {};
}

FcitxWatchConfig buildBaseFcitxWatchConfig(
    const std::filesystem::path &initial_config_home,
    const char *client_directory,
    const char *system_options_path) {
  FcitxWatchConfig watch;
  const std::string client_dir = client_directory ? client_directory : "";
  const auto current_cfg = resolveFcitxConfigHome();

  if (const auto *override_options = std::getenv("MSIME_FCITX5_OPTIONS");
      override_options && *override_options) {
    std::filesystem::path explicit_options(override_options);
    if (explicit_options.is_absolute())
      watch.files.push_back(std::move(explicit_options));
  } else {
    for (const auto &cfg_home : {current_cfg, initial_config_home}) {
      if (!cfg_home.empty() && cfg_home.is_absolute() && !client_dir.empty())
        watch.files.push_back(cfg_home / client_dir / "runtime-options.json");
    }
    if (system_options_path && *system_options_path) {
      std::filesystem::path sys_options(system_options_path);
      if (sys_options.is_absolute()) watch.files.push_back(std::move(sys_options));
    }
  }

  for (const auto &cfg_home : {current_cfg, initial_config_home}) {
    if (!cfg_home.empty() && cfg_home.is_absolute())
      watch.files.push_back(cfg_home / "fcitx5/conf/classicui.conf");
  }

  if (const auto *runtime = std::getenv("XDG_RUNTIME_DIR"); runtime && *runtime && !client_dir.empty()) {
    std::filesystem::path runtime_path(runtime);
    if (runtime_path.is_absolute())
      watch.socket_dirs.push_back(runtime_path / client_dir);
  }
  return watch;
}

void appendContextWatchPaths(
    FcitxWatchConfig &watch,
    const std::string &options_path,
    const std::string &dictionary_user_data,
    std::string_view quiesce_lease_name,
    const std::string &clipboard_path,
    std::initializer_list<const std::string *> sockets) {
  if (!options_path.empty() && options_path.front() == '/')
    watch.preference_dirs.emplace_back(options_path);
  if (!dictionary_user_data.empty() && dictionary_user_data.front() == '/' &&
      !quiesce_lease_name.empty()) {
    watch.files.push_back(std::filesystem::path(dictionary_user_data) /
                          std::string(quiesce_lease_name));
  }
  if (!clipboard_path.empty() && clipboard_path.front() == '/')
    watch.files.push_back(std::filesystem::path(clipboard_path) / "clipboard_history.json");
  for (const auto *sock : sockets) {
    if (sock && !sock->empty() && sock->front() == '/')
      watch.files.emplace_back(*sock);
  }
}

namespace {

constexpr std::uint32_t kInotifyWatchMask =
    IN_CLOSE_WRITE | IN_MODIFY | IN_ATTRIB | IN_MOVED_TO | IN_MOVED_FROM |
    IN_CREATE | IN_DELETE | IN_DELETE_SELF | IN_MOVE_SELF;

int defaultInotifyInit(int flags) { return ::inotify_init1(flags); }

bool directoryExists(const std::filesystem::path &path) {
  if (path.empty() || !path.is_absolute()) return false;
  std::error_code error;
  return std::filesystem::is_directory(path, error);
}

// 目标目录尚不存在时退回最近一层已存在的祖先目录，并记下需要等待创建的下一级名字。
bool resolveExistingAncestor(const std::filesystem::path &target_dir,
                             std::filesystem::path &ancestor,
                             std::string &next_step) {
  if (target_dir.empty() || !target_dir.is_absolute()) return false;
  std::filesystem::path current = target_dir.lexically_normal();
  std::string step;
  while (!current.empty()) {
    if (directoryExists(current)) {
      ancestor = std::move(current);
      next_step = std::move(step);
      return true;
    }
    const auto parent = current.parent_path();
    if (parent == current) break;
    step = current.filename().string();
    current = parent;
  }
  return false;
}

bool isPreferenceNoiseFile(std::string_view name) {
  return name.empty() || name.starts_with("diagnostic.log") ||
         name.ends_with(".lock");
}

bool isWatchedProviderSocket(std::string_view name) {
  return !name.empty() && name != "panel-input.sock" &&
         name.ends_with(".sock");
}

} // namespace

FcitxIdleWatch::FcitxIdleWatch(fcitx::EventLoop &loop, Callback callback,
                               std::uint64_t debounce_us,
                               std::uint64_t fallback_interval_us,
                               InotifyInitFn init_fn)
    : loop_(loop),
      callback_(std::move(callback)),
      debounce_us_(std::max<std::uint64_t>(1000, debounce_us)),
      fallback_interval_us_(std::max<std::uint64_t>(1000, fallback_interval_us)) {
  const auto init = init_fn ? init_fn : &defaultInotifyInit;
  inotify_fd_ = init(IN_NONBLOCK | IN_CLOEXEC);
  if (inotify_fd_ >= 0) {
    io_event_ = loop_.addIOEvent(
        inotify_fd_, fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) {
          handleInotifyReadable();
          return true;
        });
  }
}

FcitxIdleWatch::~FcitxIdleWatch() {
  fallback_timer_.reset();
  debounce_timer_.reset();
  io_event_.reset();
  if (inotify_fd_ >= 0) {
    for (const auto &[path, wd] : watched_by_path_) {
      (void)path;
      ::inotify_rm_watch(inotify_fd_, wd);
    }
    ::close(inotify_fd_);
    inotify_fd_ = -1;
  }
}

void FcitxIdleWatch::normalizePaths(std::vector<std::filesystem::path> &paths) {
  std::vector<std::filesystem::path> cleaned;
  cleaned.reserve(paths.size());
  for (auto &item : paths) {
    if (item.empty() || !item.is_absolute()) continue;
    auto normalized = item.lexically_normal();
    if (normalized.filename().empty() && normalized.has_parent_path() &&
        normalized != normalized.root_path()) {
      normalized = normalized.parent_path();
    }
    cleaned.push_back(std::move(normalized));
  }
  std::sort(cleaned.begin(), cleaned.end());
  cleaned.erase(std::unique(cleaned.begin(), cleaned.end()), cleaned.end());
  paths = std::move(cleaned);
}

void FcitxIdleWatch::configure(FcitxWatchConfig config) {
  normalizePaths(config.files);
  normalizePaths(config.preference_dirs);
  normalizePaths(config.socket_dirs);
  if (config == config_) {
    if (usingFallback()) syncFallbackTimer();
    return;
  }
  config_ = std::move(config);
  if (usingFallback()) {
    syncFallbackTimer();
    return;
  }
  rebuildWatches();
}

void FcitxIdleWatch::setActive(bool active) {
  if (active_ == active) return;
  active_ = active;
  if (!active_) {
    if (debounce_timer_) debounce_timer_->setEnabled(false);
    if (fallback_timer_) fallback_timer_->setEnabled(false);
    return;
  }
  if (usingFallback()) {
    syncFallbackTimer();
  } else {
    rebuildWatches();
  }
}

bool FcitxIdleWatch::hasArmedTimer() const noexcept {
  return (debounce_timer_ && debounce_timer_->isEnabled()) ||
         (fallback_timer_ && fallback_timer_->isEnabled());
}

void FcitxIdleWatch::rebuildWatches() {
  if (inotify_fd_ < 0) return;
  std::map<std::filesystem::path, WatchedDirSpec> desired;

  for (const auto &file : config_.files) {
    const auto leaf = file.filename().string();
    const auto parent = file.parent_path();
    if (leaf.empty() || parent.empty()) continue;
    std::filesystem::path ancestor;
    std::string step;
    if (!resolveExistingAncestor(parent, ancestor, step)) continue;
    if (step.empty()) {
      desired[ancestor].exact_files.insert(leaf);
    } else {
      desired[ancestor].ancestor_steps.insert(step);
    }
  }

  for (const auto &dir : config_.preference_dirs) {
    std::filesystem::path ancestor;
    std::string step;
    if (!resolveExistingAncestor(dir, ancestor, step)) continue;
    if (step.empty()) {
      desired[ancestor].preference_dir = true;
    } else {
      desired[ancestor].ancestor_steps.insert(step);
    }
  }

  for (const auto &dir : config_.socket_dirs) {
    std::filesystem::path ancestor;
    std::string step;
    if (!resolveExistingAncestor(dir, ancestor, step)) continue;
    if (step.empty()) {
      desired[ancestor].socket_dir = true;
    } else {
      desired[ancestor].ancestor_steps.insert(step);
    }
  }

  for (auto it = watched_by_path_.begin(); it != watched_by_path_.end();) {
    if (desired.find(it->first) == desired.end()) {
      const int wd = it->second;
      ::inotify_rm_watch(inotify_fd_, wd);
      watched_by_wd_.erase(wd);
      it = watched_by_path_.erase(it);
    } else {
      ++it;
    }
  }

  for (auto &[dir, spec] : desired) {
    const auto existing = watched_by_path_.find(dir);
    if (existing != watched_by_path_.end()) {
      auto wd_it = watched_by_wd_.find(existing->second);
      if (wd_it != watched_by_wd_.end()) wd_it->second.spec = std::move(spec);
      continue;
    }
    const int wd = ::inotify_add_watch(inotify_fd_, dir.c_str(), kInotifyWatchMask);
    if (wd < 0) continue;
    watched_by_path_[dir] = wd;
    watched_by_wd_[wd] = WatchedDirEntry{dir, std::move(spec)};
  }
}

void FcitxIdleWatch::handleInotifyReadable() {
  if (inotify_fd_ < 0) return;
  alignas(struct inotify_event) char buffer[4096];
  bool need_rebuild = false;
  bool matched = false;

  for (;;) {
    const ssize_t bytes = ::read(inotify_fd_, buffer, sizeof(buffer));
    if (bytes < 0) {
      if (errno == EINTR) continue;
      break;
    }
    if (bytes == 0) break;

    std::size_t offset = 0;
    while (offset + sizeof(struct inotify_event) <= static_cast<std::size_t>(bytes)) {
      const auto *event =
          reinterpret_cast<const struct inotify_event *>(buffer + offset);
      offset += sizeof(struct inotify_event) + event->len;

      if ((event->mask & IN_Q_OVERFLOW) != 0) {
        need_rebuild = true;
        matched = true;
        continue;
      }
      if ((event->mask & IN_IGNORED) != 0) {
        const auto wd_it = watched_by_wd_.find(event->wd);
        if (wd_it != watched_by_wd_.end()) {
          watched_by_path_.erase(wd_it->second.directory);
          watched_by_wd_.erase(wd_it);
        }
        need_rebuild = true;
        continue;
      }

      const auto wd_it = watched_by_wd_.find(event->wd);
      if (wd_it == watched_by_wd_.end()) continue;
      const auto &spec = wd_it->second.spec;

      if ((event->mask & (IN_DELETE_SELF | IN_MOVE_SELF | IN_UNMOUNT)) != 0) {
        need_rebuild = true;
        matched = true;
      }

      const std::string_view name =
          event->len > 0 ? std::string_view(event->name) : std::string_view{};
      if (name.empty()) continue;

      if (spec.ancestor_steps.contains(std::string(name))) {
        need_rebuild = true;
        matched = true;
      }
      if (spec.exact_files.contains(std::string(name))) {
        matched = true;
      }
      if (spec.preference_dir && !isPreferenceNoiseFile(name)) {
        matched = true;
      }
      if (spec.socket_dir && isWatchedProviderSocket(name)) {
        matched = true;
      }
    }
  }

  if (need_rebuild) rebuildWatches();
  if (matched && active_) armDebounce();
}

void FcitxIdleWatch::armDebounce() {
  const std::uint64_t deadline = fcitx::now(CLOCK_MONOTONIC) + debounce_us_;
  if (!debounce_timer_) {
    debounce_timer_ = loop_.addTimeEvent(
        CLOCK_MONOTONIC, deadline, 0,
        [this](fcitx::EventSourceTime *timer, std::uint64_t) {
          // 触发后立即关闭，避免 `sd-event` 时间源在未推进截止时刻时重复触发。
          timer->setEnabled(false);
          if (active_ && callback_) callback_();
          return true;
        });
  } else {
    debounce_timer_->setTime(deadline);
    debounce_timer_->setOneShot();
  }
}

void FcitxIdleWatch::syncFallbackTimer() {
  if (!usingFallback()) return;
  const bool should_run = active_ && !config_.empty();
  if (!should_run) {
    if (fallback_timer_) fallback_timer_->setEnabled(false);
    return;
  }
  const std::uint64_t deadline = fcitx::now(CLOCK_MONOTONIC) + fallback_interval_us_;
  if (!fallback_timer_) {
    fallback_timer_ = loop_.addTimeEvent(
        CLOCK_MONOTONIC, deadline, 10'000,
        [this](fcitx::EventSourceTime *timer, std::uint64_t) {
          if (!active_ || config_.empty()) {
            timer->setEnabled(false);
            return true;
          }
          timer->setNextInterval(fallback_interval_us_);
          timer->setOneShot();
          if (callback_) callback_();
          return true;
        });
  } else if (!fallback_timer_->isEnabled()) {
    fallback_timer_->setTime(deadline);
    fallback_timer_->setOneShot();
  }
}

} // namespace msime::fcitx_host
