#pragma once
#include <algorithm>
#include <array>
#include <optional>
#include <string_view>

namespace msime::windows {
// 共享偏好 `candidate_font_weight`（client-core 的 `CandidateFontWeight`）：文档里的取值、DirectWrite 用的 OpenType 字重和设置页上的名字。缺这个键就是常规，等于这个设置出现之前的画法。这份表抄自 Rust，scripts/test-candidate-font-weight-parity.py 核对它与 Rust 和另外两端一致。
struct CandidateFontWeightOption {
  std::string_view id;
  unsigned weight;
  std::wstring_view label;
};

inline constexpr unsigned candidate_regular_font_weight = 400;

inline constexpr std::array<CandidateFontWeightOption, 4> candidate_font_weights{{
    {"regular", 400, L"常规"},
    {"medium", 500, L"中等"},
    {"semibold", 600, L"半粗"},
    {"bold", 700, L"粗体"},
}};

// 文档里的取值换成字重；不认识的取值为空，由调用方决定怎么处理。
inline std::optional<unsigned> candidate_font_weight(std::string_view id) {
  for (const auto &option : candidate_font_weights)
    if (option.id == id)
      return option.weight;
  return std::nullopt;
}

inline bool candidate_font_weight_valid(unsigned weight) {
  return std::any_of(candidate_font_weights.begin(), candidate_font_weights.end(),
                     [weight](const auto &option) { return option.weight == weight; });
}

// 预编辑行一直是半粗，候选文字选得更粗时跟到同一档（`CandidateFontWeight::preedit_value`）。
inline constexpr unsigned candidate_preedit_font_weight(unsigned weight) {
  return weight > 600 ? weight : 600;
}
} // namespace msime::windows
