#pragma once

#include <nlohmann/json.hpp>

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>

namespace msime::linux_host {

// 应用例外（共享偏好 `app_input_mode_rules`）在 Linux 上的读法：键是程序名，值是焦点从别的程序进入它时的起始模式（"chinese" 或 "english"）。程序名取自输入法框架：Fcitx5 是 `InputContext::program()`（GTK、Qt 的输入法模块报告的程序名，X11 和 Wayland 下都有，前端不报告时为空）；IBus 1.5.27 起在 `focus_in_id` 里给出客户端名，只有 GTK 输入法模块按 `gtk3-im:<程序名>` 的格式带上程序名，Qt 插件、XIM 和更早的 IBus 都是匿名的（见 `ibus_app_rule_program`）。共享文档里同一张表也装着 macOS 的 bundle id 和 Windows 的 `.exe` 基名，它们一般不会和 Linux 程序名相等，在这里自然不命中。
//
// 比较只折叠 ASCII 字母，和偏好库按 `to_ascii_lowercase` 去重、Windows 按进程基名不分大小写比较的结果一致。
inline bool same_app_rule_program(std::string_view left, std::string_view right) {
  if (left.size() != right.size())
    return false;
  for (std::size_t index = 0; index < left.size(); ++index) {
    const auto fold = [](char value) {
      return value >= 'A' && value <= 'Z' ? static_cast<char>(value - 'A' + 'a') : value;
    };
    if (fold(left[index]) != fold(right[index]))
      return false;
  }
  return true;
}

// `preferences` 里命中 `program` 的规则：true 是中文、false 是英文，没有规则时为空。程序名为空（框架没报告）时一律不命中。规则表不是对象、值不是 chinese/english 的条目一律跳过：偏好库已经校验过文档，这里只防一份手改坏的文件。
inline std::optional<bool> app_input_mode_rule(const nlohmann::json &preferences,
                                               std::string_view program) {
  if (program.empty() || !preferences.is_object())
    return std::nullopt;
  const auto rules = preferences.find("app_input_mode_rules");
  if (rules == preferences.end() || !rules->is_object())
    return std::nullopt;
  for (const auto &[app, mode] : rules->items()) {
    if (!mode.is_string() || !same_app_rule_program(app, program))
      continue;
    const auto &value = mode.get_ref<const std::string &>();
    if (value == "chinese")
      return true;
    if (value == "english")
      return false;
  }
  return std::nullopt;
}

// IBus 客户端名里的程序名。GTK 输入法模块建上下文时把客户端名写成 `gtk-im:`、`gtk3-im:` 或 `gtk4-im:` 加 `g_get_prgname()`（ibus 的 client/gtk2/ibusimcontext.c），只认这种格式；Qt 插件的 `QIBusInputContext`、XIM 的 `xim`、IBus 自己的 `fake` 等不带程序名，返回空，规则对它们不生效。
inline std::string ibus_app_rule_program(std::string_view client) {
  const auto colon = client.find(':');
  if (colon == std::string_view::npos)
    return {};
  const auto kind = client.substr(0, colon);
  if (kind != "gtk-im" && kind != "gtk3-im" && kind != "gtk4-im")
    return {};
  return std::string(client.substr(colon + 1));
}

// 一次停留：焦点留在同一个程序里（几个窗口、一个窗口里的几个输入框）算同一次，进入另一个程序才是新的一次。用户在一次停留里手动切换中英文后，规则让位到离开这个程序为止，和 macOS、Windows 一样。焦点到了一个没有程序名的上下文时由调用方判断：确定是另一个客户端（IBus 的 Qt 插件、XIM、GNOME Shell，Fcitx5 不报告程序名的前端）就调 `leave`，这次停留结束；认不出换没换客户端（IBus 先匿名聚焦、稍后补名的协商，IBus 自己的 `fake` 上下文）就调 `enter("")`，停留保持不变。
class AppInputModeRuleStay {
public:
  // 焦点进入 `program`。返回这次是不是新的停留。
  bool enter(std::string_view program) {
    if (program.empty() || same_app_rule_program(program_, program))
      return false;
    program_ = std::string(program);
    yielded_ = false;
    return true;
  }

  // 焦点进入一个没有程序名、但确定不是当前程序的客户端：和 macOS、Windows 切到别的程序一样，这次停留结束，回到原来的程序时规则重新生效。
  void leave() {
    program_.clear();
    yielded_ = false;
  }

  // 用户在 `program` 里手动切换了中英文。
  void yield(std::string_view program) {
    if (!program.empty() && same_app_rule_program(program_, program))
      yielded_ = true;
  }

  // 焦点在 `program` 时规则还要不要套用：在这次停留里还没让位。
  bool applies(std::string_view program) const {
    return !program.empty() && !(yielded_ && same_app_rule_program(program_, program));
  }

private:
  std::string program_;
  bool yielded_ = false;
};

// `program` 这次聚焦要从哪个模式开始：有规则且这次停留里还没让位时是规则的模式，否则为空，由调用方照旧按作用域和记忆决定。
inline std::optional<bool> app_input_mode_rule_start(const nlohmann::json &preferences,
                                                     std::string_view program,
                                                     const AppInputModeRuleStay &stay) {
  if (!stay.applies(program))
    return std::nullopt;
  return app_input_mode_rule(preferences, program);
}

}  // namespace msime::linux_host
