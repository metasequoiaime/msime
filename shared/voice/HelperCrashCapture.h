#pragma once

#include <memory>

namespace msime::voice {

// 仅由安装包启用；平台适配拥有真实偏好与遥测目录，独立构建的语音库不猜测产品身份。
class HelperCrashCapture final {
public:
  HelperCrashCapture() noexcept;
  ~HelperCrashCapture();
  HelperCrashCapture(const HelperCrashCapture &) = delete;
  HelperCrashCapture &operator=(const HelperCrashCapture &) = delete;

private:
  struct State;
  std::unique_ptr<State> state_;
};

} // namespace msime::voice
