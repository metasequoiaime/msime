#pragma once
#include <cstdint>
#include <cstdio>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace msime::windows {
// 一个界面组件放弃工作的位置：固定的阶段标签，或出错时正在处理的窗口消息号，再加上捕获当时的 Win32 错误码。诊断日志只能写固定标签和数字，所以这里不收异常文本，按键、上屏文字和候选都没有路径进入日志。
struct ComponentFailureSite {
  // 字符串字面量，例如 "refresh"；为空表示出错在窗口消息里，看 message。
  const char *stage = nullptr;
  uint32_t message = 0;
  uint32_t error = 0;
};
inline ComponentFailureSite failure_at_stage(const char *stage, uint32_t error) {
  return ComponentFailureSite{stage, 0, error};
}
inline ComponentFailureSite failure_in_message(uint32_t message, uint32_t error) {
  return ComponentFailureSite{nullptr, message, error};
}
// "refresh, error 5" 或 "window message 0x000F, error 0"。
inline std::string describe_failure_site(const ComponentFailureSite &site) {
  char text[96];
  if (site.stage)
    std::snprintf(text, sizeof(text), "%s, error %lu", site.stage, static_cast<unsigned long>(site.error));
  else
    std::snprintf(text, sizeof(text), "window message 0x%04lX, error %lu", static_cast<unsigned long>(site.message), static_cast<unsigned long>(site.error));
  return text;
}
// 组件名加上失败位置，例如 "candidate window failed (window message 0x000F, error 0)"；位置未知时只有名字。
inline std::string component_failure(std::string_view component, const std::optional<ComponentFailureSite> &site) {
  std::string line(component);
  line += " failed";
  if (site) {
    line += " (";
    line += describe_failure_site(*site);
    line += ')';
  }
  return line;
}
// 主循环自己结束（不是停止或重启请求）时写进诊断日志的那一行：没有组件失败是正常停止，否则逐个列出失败的组件，支持日志不必再猜是哪一个。
inline std::string server_stop_line(const std::vector<std::string> &failures) {
  if (failures.empty())
    return "Server stopping";
  std::string line = "Server stopping: ";
  for (size_t i = 0; i < failures.size(); ++i) {
    if (i)
      line += "; ";
    line += failures[i];
  }
  return line;
}
} // namespace msime::windows
