#pragma once
#include "InputModeHudLayout.h"
#include "../system/FocusLease.h"
#include <chrono>
#include <mutex>
#include <utility>

namespace msime::windows {
inline bool input_hud_same_focus(const FocusLease &a, const FocusLease &b) {
  return a.epoch == b.epoch && a.token == b.token && same_ticket(a.transport, b.transport);
}

// 工作线程只复制最后一次成功操作；界面线程统一定位、计时和判断开关。
// 绑定会话的提示不得跟到别的输入框；全局偏好操作绑定原前台窗口。
class InputStateHudFeedback final {
public:
  using Clock = std::chrono::steady_clock;
  void publish(InputStateHudText text, std::optional<FocusLease> lease = {},
               uintptr_t foreground = 0, Clock::time_point now = Clock::now()) {
    std::lock_guard lock(mutex_);
    latest_ = Notice{std::move(text), lease, foreground, now};
  }
  std::optional<InputStateHudText> take(const std::optional<FocusLease> &lease,
                                      uintptr_t foreground,
                                      Clock::time_point now = Clock::now()) {
    std::lock_guard lock(mutex_);
    auto notice = std::exchange(latest_, std::nullopt);
    if (!notice || now - notice->at > std::chrono::seconds(1) ||
        (notice->foreground && notice->foreground != foreground) ||
        (notice->lease && (!lease || !input_hud_same_focus(*notice->lease, *lease))))
      return std::nullopt;
    return std::move(notice->text);
  }
private:
  struct Notice {
    InputStateHudText text;
    std::optional<FocusLease> lease;
    uintptr_t foreground;
    Clock::time_point at;
  };
  std::mutex mutex_;
  std::optional<Notice> latest_;
};

// 只在输入队列使用。快照建立基线，只有 TIP 确认的显式切换事件产生提示。
class InputStateHudModeTracker final {
public:
  std::optional<InputStateHudText> observe(const FocusLease &lease,
      std::optional<bool> fullwidth, std::optional<bool> punctuation, bool user_event) {
    if (!lease_ || !input_hud_same_focus(*lease_, lease)) {
      lease_ = lease;
      fullwidth_.reset();
      punctuation_.reset();
    }
    std::optional<InputStateHudText> result;
    if (user_event && fullwidth_ && fullwidth && fullwidth_ != fullwidth)
      result = fullwidth_hud(*fullwidth);
    if (user_event && punctuation_ && punctuation && punctuation_ != punctuation)
      result = punctuation_hud(*punctuation);
    if (fullwidth) fullwidth_ = fullwidth;
    if (punctuation) punctuation_ = punctuation;
    return result;
  }
private:
  std::optional<FocusLease> lease_;
  std::optional<bool> fullwidth_, punctuation_;
};
} // namespace msime::windows
