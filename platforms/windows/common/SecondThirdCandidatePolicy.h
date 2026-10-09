#pragma once

#include "InputSchemeTraits.h"
#include <cstddef>
#include <cstdint>
#include <optional>

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
