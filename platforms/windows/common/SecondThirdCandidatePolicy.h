#pragma once

#include "InputSchemeTraits.h"
#include <cstddef>
#include <cstdint>
#include <optional>
#include <string_view>

namespace msime::windows {

// 「二三候选」键位：开启后组字时 ';' 选当前页第二个候选，'\'' 选第三个，和数字键 2、3 一样。偏好在共享偏好的 `second_third_candidate` 里，Server 从偏好快照读，TIP 从 SecondThirdCandidateChanged 推送读。
//
// TIP 把这两个键归类成数字选词（FUNCTION_SELECT_BY_NUMBER），Server 按同一条规则选对应的候选，两边必须对同一个键给出同一个答案，否则 TIP 等的是选词回复，Server 却当标点处理。所以判定集中在这里，两边各自算出自己知道的那几个状态后调用同一个函数。
//
// 不接管的情形：
// - 没有在组字：两个键照常是标点。
// - 日文：';' 和 '\'' 是罗马字输入的一部分（n' 打出ん）。韩文、注音、越南文和藏文在 TIP 自己的宿主会话里组字（scheme::AlwaysInlinePreedit），候选列表也不是一直开着。
// - Engine 自己的英文模式：英文单词里的 '\'' 和 ';' 是要打出来的字符。
// - 这个键在当前状态下是输入（engine_input）：网址模式拼写 ';' 和 '\''，微软双拼在声母后把 ';' 当作韵母 ing。
//
// 键要同时对上虚拟键码和打出的字符，与以词定字的 '[' ']' 相同：别的键盘布局在这两个键上打出别的字符时不接管，Shift 打出的 ':' 和 '"' 也不接管。

// ';'（VK_OEM_1）和 '\''（VK_OEM_7）各自选第几个候选，从 0 数起。
inline std::optional<std::size_t> second_third_candidate_slot(std::uint32_t keycode, std::uint32_t text) {
  if (keycode == 0xBAu && text == ';')
    return 1;
  if (keycode == 0xDEu && text == '\'')
    return 2;
  return std::nullopt;
}

// TIP 的候选处理器还要把数字键和二三候选键映射到当前页下标。数字键按虚拟键码识别，以保留 V 模式和非美式布局下的既有规则；二三候选键同时校验字符。
inline std::optional<std::size_t>
candidate_selection_slot(std::uint32_t keycode, std::uint32_t text) {
  if (const auto special = second_third_candidate_slot(keycode, text))
    return special;
  if (keycode >= static_cast<std::uint32_t>('1') &&
      keycode <= static_cast<std::uint32_t>('9'))
    return keycode - static_cast<std::uint32_t>('1');
  if (keycode >= 0x61u && keycode <= 0x69u)
    return keycode - 0x61u;
  return std::nullopt;
}

// TIP 的候选处理器在本地 presenter 里选中第几项（CCandidateRange::GetIndex），page_size 是一页的槽位数。
//
// engine_page：presenter 持有 TIP 宿主会话的真实一页（候选带着宿主会话的身份）。这时 TIP 用选中的那一项向宿主会话选词并上屏，必须选键对应的位置；超出一页的键什么也不选。
//
// 否则 presenter 只是一份最小镜像：没有宿主会话时只有键入的原文这一项，UI-less 宿主拿到的是 Server 那一页的文字。上屏的候选来自 Server 对这次按键的选词回复，本地选中哪一项都不影响结果，但必须选中一项，TIP 才会去读那份回复（_HandleCandidateFinalize）。所以这里一律选第一项，和二三候选之前数字键的做法相同；按真实位置去选，一项的镜像会越界，Server 已经选好的回复就再也没人读，两边从此分歧。
inline std::optional<std::size_t>
tip_candidate_selection_index(std::uint32_t keycode, std::uint32_t text, bool engine_page, std::size_t page_size) {
  if (!engine_page)
    return 0;
  const auto slot = candidate_selection_slot(keycode, text);
  if (!slot || *slot >= page_size)
    return std::nullopt;
  return slot;
}

// 微软双拼把 ';' 当作韵母 ing：光标前、上一个 '\'' 之后的字母数是奇数（刚打完声母）时，它是输入。与 EditPolicy.h 的 edit_kind 和 TIP 归类按键时的同一条判断相同。
template <typename Text>
bool microsoft_shuangpin_final_position(const Text &editing, std::size_t caret) {
  if (caret > editing.size())
    caret = editing.size();
  std::size_t start = caret;
  while (start > 0 && editing[start - 1] != '\'')
    --start;
  return (caret - start) % 2 == 1;
}

// Engine 在当前状态下把这个键当作输入：视图的 spelling_symbols 列出了它（网址模式和 V 模式拼写的符号），或者它是微软双拼声母后的韵母 ing。Server 读自己会话的视图；TIP 有宿主会话时读宿主会话的视图（组字归宿主会话，旧的按键缓冲是空的），没有时用按键缓冲和本地记下的模式。两边用同一个函数，对同一个键的判断才一致。spelling_symbols 的判断与 Server 的 spelled_by_engine（src/input/EditPolicy.h）相同。
template <typename Text>
bool second_third_candidate_engine_input(std::string_view spelling_symbols, std::uint32_t text,
                                         bool microsoft_shuangpin, const Text &editing, std::size_t caret) {
  const bool spelled = text >= 0x21u && text <= 0x7Eu &&
                       spelling_symbols.find(static_cast<char>(text)) != std::string_view::npos;
  return spelled || (text == ';' && microsoft_shuangpin && microsoft_shuangpin_final_position(editing, caret));
}

// 这个方案的组字里两个键能不能选候选。
constexpr bool second_third_candidate_scheme(int scheme) {
  return scheme != scheme::Japanese && !scheme::AlwaysInlinePreedit(scheme);
}

// 组字时这个键是不是选二三候选；是的话给出候选在当前页里的位置。modifiers 只算 Shift、Ctrl、Alt，任何一个按着都不接管。
inline std::optional<std::size_t>
second_third_candidate_selection(bool enabled, std::uint32_t keycode, std::uint32_t text, std::uint32_t modifiers,
                                 bool composing, int scheme, bool dedicated_english, bool engine_input) {
  if (!enabled || modifiers != 0 || !composing || dedicated_english || engine_input ||
      !second_third_candidate_scheme(scheme))
    return std::nullopt;
  return second_third_candidate_slot(keycode, text);
}

} // namespace msime::windows
