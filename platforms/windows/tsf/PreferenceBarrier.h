#pragma once
#include "../../../shared/contracts/preference_transition.h"
#include <algorithm>
#include <cstdint>

namespace msime::tsf {
// 仅由 TIP 所属线程持有；worker 通过有界邮箱交接不可变通知。
class PreferenceBarrier final {
public:
  void begin(std::uint64_t focus) { focus_ = focus; request_ = FANY_IME_NO_REQUEST_ID; active_ = true; }
  void sent(std::uint64_t request) { request_ = request; }
  bool receive(const FanyImePreferencePipe::Transition &value, std::uint64_t current_focus) {
    if (value.request != 0) {
      if (!active_ || request_ != value.request || focus_ != value.focus || current_focus != value.focus) return false;
      request_ = 0;
    }
    target_ = (std::max)(target_, value.revision);
    active_ = active_ || target_ > applied_;
    return true;
  }
  bool active() const { return active_; }
  bool awaiting_reply() const { return request_ != 0; }
  std::uint64_t target() const { return target_; }
  std::uint64_t applied() const { return applied_; }
  bool complete(std::uint64_t revision) {
    if (!active_ || request_ != 0 || revision < target_ || revision < applied_) return false;
    applied_ = revision; active_ = false; return true;
  }
  void reset() { request_ = focus_ = 0; target_ = applied_; active_ = false; }
  // 引擎会话随 Deactivate 一起销毁时调用：applied_ 记的是旧会话套过的修订，新会话没有套过任何修订。reset() 留着它，同一个 TIP 对象再次 ActivateEx 后偏好定时器只在修订号变大时才套偏好，用户不改设置就永远不套，新会话停在 runtime-options.json 的默认方案（#10867）。清零之后下一轮定时器把当前修订当作新目标重新走一次有序应用；还在路上的旧请求回执因为屏障不在进行中，照样被 receive 拒绝。
  void forget_applied() { reset(); target_ = applied_ = 0; }
private:
  std::uint64_t request_ = 0, focus_ = 0, target_ = 0, applied_ = 0;
  bool active_ = false;
};
} // namespace msime::tsf
