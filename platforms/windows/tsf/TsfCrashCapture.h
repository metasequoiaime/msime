#pragma once

namespace msime::tsf {

// 每个 TSF 实例持有一个租约；最后一个租约释放后才注销 WER 并解除武装。
class TsfCrashCapture final {
public:
  TsfCrashCapture() noexcept;
  ~TsfCrashCapture() noexcept;
  TsfCrashCapture(const TsfCrashCapture &) = delete;
  TsfCrashCapture &operator=(const TsfCrashCapture &) = delete;
private:
  bool lease_ = false;
};

} // namespace msime::tsf
