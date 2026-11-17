#pragma once

#include <cstdint>
#include <functional>
#include <memory>
#include <string>

namespace fcitx {
class EventLoop;
class EventSource;
} // namespace fcitx

namespace msime::linux_host {

// 将 Fcitx5 的模式提示排到当前输入事件返回之后，避免客户端在按键栈内重入 UI 更新。
class ModeHudDeferredUpdate {
public:
  using Callback = std::function<void(const std::string &label)>;

  explicit ModeHudDeferredUpdate(fcitx::EventLoop &loop);
  ~ModeHudDeferredUpdate();
  ModeHudDeferredUpdate(const ModeHudDeferredUpdate &) = delete;
  ModeHudDeferredUpdate &operator=(const ModeHudDeferredUpdate &) = delete;

  void schedule(std::string label, Callback callback);
  void cancel();

private:
  fcitx::EventLoop &loop_;
  std::unique_ptr<fcitx::EventSource> event_;
  uint64_t generation_ = 0;
};

} // namespace msime::linux_host
