#pragma once

#include <string_view>

namespace msime::linux_host {

// Footer label for the Engine's local input mode, shared by the IBus auxiliary text and the Fcitx5 aux-down line. The keys are the names the Engine emits in `local_mode` (crates/engine/src/types.rs LocalInputMode::name); "none" and any unknown name have no label.
inline const char *candidate_local_mode_label(std::string_view mode) {
  struct Entry {
    std::string_view name;
    const char *label;
  };
  static constexpr Entry entries[] = {
      {"unicode", "U+"},          {"date_time", "日期时间"},
      {"quick_phrase", "短语"},   {"emoji", "Emoji"},
      {"kaomoji", "颜文字"},      {"super_jianpin", "简拼"},
      {"temporary_english", "EN"}, {"temporary_japanese", "日文"},
      {"expression", "计算"},     {"command", "指令"},
      {"mention", "@"},           {"url", "网址"}};
  for (const auto &entry : entries)
    if (entry.name == mode)
      return entry.label;
  return nullptr;
}

} // namespace msime::linux_host
