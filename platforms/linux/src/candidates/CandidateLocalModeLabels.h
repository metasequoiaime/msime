#pragma once

#include <string_view>

namespace msime::linux_host {

// IBus 辅助文本与 Fcitx5 底栏共用的本地模式标签，键与引擎 `local_mode` 名称一致；默认和未知模式不显示标签。
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
      {"mention", "@"},           {"url", "网址"},
      {"chaizi", "拆字"},         {"symbol", "符号"}};
  for (const auto &entry : entries)
    if (entry.name == mode)
      return entry.label;
  return nullptr;
}

} // namespace msime::linux_host
