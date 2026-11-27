// IBus 候选维护动作的属性名编码：Action/session/generation/index。
// 引擎分发（ClientEngine::property_activate）与 GNOME Shell 扩展点击回传共用同一份
// 编码，扩展只需把收到的 action_name 原样交给 IBus panelService.property_activate，
// 引擎即可按 session/generation/index 定位到当时渲染的那份候选并核对身份。
#pragma once

#include <cstddef>
#include <cstdint>
#include <string>

namespace msime::linux_host {

inline std::string candidate_action_name(const char *action, std::uint64_t session,
                                         std::uint64_t generation, std::size_t index) {
  return std::string(action) + "/" + std::to_string(session) + "/" +
         std::to_string(generation) + "/" + std::to_string(index);
}

}  // namespace msime::linux_host
