#pragma once
#include <cstdint>
#include <limits>
#include <string>
#include <string_view>

// 来源：msime-windows e46294308df5a93d5f628878543a6a373cde3ccc，engine/contracts/uiless_page.h（GPL-3.0）。
// 同步回复和后台推送共用：preedit、逗号分隔的当前页、页内高亮、候选代次，以制表符分隔。
// 两条管道可能乱序，较旧的代次不能覆盖新页；旧 Server 没有代次时按 0 接受。
namespace FanyImeUiLessPage {
struct Payload {
  std::wstring preedit;
  std::wstring page;
  int selection = 0;
  std::uint64_t generation = 0;
  bool hasPage = false;
};
inline bool Number(std::wstring_view text, std::uint64_t &value) {
  value = 0;
  if (text.empty())
    return false;
  for (const auto ch : text) {
    if (ch < L'0' || ch > L'9' ||
        value >
            ((std::numeric_limits<std::uint64_t>::max)() - (ch - L'0')) / 10)
      return false;
    value = value * 10 + (ch - L'0');
  }
  return true;
}
inline Payload Parse(std::wstring_view text) {
  Payload result;
  const auto first = text.find(L'\t');
  result.preedit = text.substr(0, first);
  if (first == std::wstring_view::npos)
    return result;
  const auto second = text.find(L'\t', first + 1);
  result.page = text.substr(first + 1, second == std::wstring_view::npos
                                           ? second
                                           : second - first - 1);
  // 最早的 Server 只有 preedit 和 page，高亮默认为首项。
  if (second == std::wstring_view::npos) {
    result.hasPage = true;
    return result;
  }
  const auto third = text.find(L'\t', second + 1);
  std::uint64_t selection = 0;
  if (!Number(text.substr(second + 1, third == std::wstring_view::npos
                                          ? third
                                          : third - second - 1),
              selection) ||
      selection > static_cast<std::uint64_t>((std::numeric_limits<int>::max)()))
    return result;
  if (third != std::wstring_view::npos &&
      !Number(text.substr(third + 1), result.generation))
    return result;
  result.selection = static_cast<int>(selection);
  result.hasPage = true;
  return result;
}
inline bool IsCurrent(std::uint64_t incoming, std::uint64_t shown) {
  return incoming == 0 || incoming >= shown;
}
} // namespace FanyImeUiLessPage
