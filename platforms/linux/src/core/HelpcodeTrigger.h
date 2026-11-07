#pragma once

#include <nlohmann/json.hpp>

#include <cstddef>
#include <cstdint>
#include <string>

namespace msime::linux_host {

// 辅助码前导键不在按键原文里（视图的 `helpcode_trigger`，见 crates/input-runtime/src/types.rs 的 `View`，#10731）：按下 `;` 之后 `editing_text` 仍是 `ni`，用户在行内看不见自己已经进了辅助码。行内画按键原文（`tsf_preedit_style` 为 raw）的宿主用它把前导键插回 `position` 处，光标不在它前面时跟着后移；`preedit` 已经带着前导键，画拼音的样式不调这个。字段缺失、类型不对或位置越界时什么也不做。按键原文是 ASCII，同一个字节偏移在 Fcitx5（按字节）和 IBus（按标量）里都成立。
inline void insert_helpcode_trigger(const nlohmann::json &view, std::string &editing, std::size_t &caret) {
  if (!view.is_object()) return;
  const auto trigger = view.find("helpcode_trigger");
  if (trigger == view.end() || !trigger->is_object()) return;
  const auto key = trigger->find("key");
  const auto position = trigger->find("position");
  if (key == trigger->end() || !key->is_string() || position == trigger->end() || !position->is_number_integer())
    return;
  // 解析出来的非负整数是无符号的，宿主自己拼的视图里是有符号的，两种都认。
  if (!position->is_number_unsigned() && position->get<std::int64_t>() < 0) return;
  const auto at = position->get<std::uint64_t>();
  const auto &text = key->get_ref<const std::string &>();
  if (text.empty() || at > editing.size()) return;
  editing.insert(static_cast<std::size_t>(at), text);
  if (caret >= at) caret += text.size();
}

} // namespace msime::linux_host
