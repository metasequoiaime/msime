#pragma once
#include "../common/TypingSource.h"
#include <optional>

namespace msime::tsf {
// 复用引擎响应已经解析出的当前视图；不读取上一键的 commit_context。
class KeyTypingSource final {
public:
  void reset() noexcept { current_.reset(); }
  void observe(int scheme, bool nine_key, bool dedicated,
               std::string_view local_mode, std::string_view profile) noexcept {
    Current next;
    next.scheme = scheme;
    next.dedicated = dedicated;
    next.source = msime::windows::resolve_typing_source(
        scheme, nine_key, dedicated, local_mode, profile);
    current_ = next;
  }
  msime::windows::TypingSource source(int configured_scheme, bool dedicated,
                                      bool chinese) const noexcept {
    if (!chinese)
      return msime::windows::TypingSource::English;
    if (!current_)
      return msime::windows::TypingSource::Unknown;
    // 物理键统计先于偏好重载。设置已经切换方案或专用英文时，旧视图不再代表本键。
    if (current_ && current_->scheme == configured_scheme &&
        current_->dedicated == dedicated)
      return current_->source;
    return msime::windows::resolve_typing_source(configured_scheme, false,
                                                 dedicated, "none", "xiaohe");
  }

private:
  struct Current {
    int scheme = -1;
    bool dedicated = false;
    msime::windows::TypingSource source = msime::windows::TypingSource::Unknown;
  };
  std::optional<Current> current_;
};
} // namespace msime::tsf
