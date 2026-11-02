#pragma once
#include "CandidateSources.h"
#include "InputSchemeTraits.h"
#include "msime_client.h"
#include "../../../../shared/input/JapaneseConversion.h"
#include <cstddef>
#include <cstdint>
#include <optional>

namespace msime::windows {
// 日语方案的空格是「変換」：第一次按开始转换、不上屏，之后每按一次移到下一个候选，过了本页末尾回到第一个；回车上屏停在的那一个。规则在 shared/input/JapaneseConversion.h，与 macOS、Linux 相同。Windows 上候选归 Server，所以空格由 Server 的 ReplyComposer 按这里的结果在自己的会话里执行，TIP 只从回复的类型知道转换已经开始（tsf/Global/JapaneseConversionPolicy.h）。纯计算，主机上的单测直接包含。

// 这个空格归不归状态机：日语方案、没有修饰键、正在组字。UILess（游戏自己画候选）照原来的规则让空格上屏：那里的回复是给宿主画的组字串，不是导航回执，TIP 读到它会把组字当成上屏内容。
inline bool japanese_space_applies(int scheme, uint32_t key, uint32_t modifiers, bool uiless, bool composing) {
  return scheme == scheme::Japanese && key == 0x20 && modifiers == 0 && !uiless && composing;
}

// 状态机的回答在 Server 会话里怎么执行。`command` 为空表示开始转换：首个候选本来就高亮着，什么都不用动，只回一个不上屏的导航回执。
struct JapaneseSpaceStep {
  std::optional<uint32_t> command;
};
// 只有开始、步进、回到首条三种回答由空格产生；不接管（唯一候选是 Fallback）时返回空，空格照常上屏高亮候选。
inline std::optional<JapaneseSpaceStep> japanese_space_step(input::JapaneseConversion::Action action) {
  using Action = input::JapaneseConversion::Action;
  switch (action) {
  case Action::Start:
    return JapaneseSpaceStep{std::nullopt};
  case Action::StepNext:
    return JapaneseSpaceStep{static_cast<uint32_t>(MSIME_NEXT_CANDIDATE)};
  case Action::StepFirst:
    return JapaneseSpaceStep{static_cast<uint32_t>(MSIME_FIRST_CANDIDATE)};
  case Action::None:
  case Action::CommitCandidate:
  case Action::CommitReading:
    break;
  }
  return std::nullopt;
}

// 空格数的行：候选窗画出来、高亮能停的那些。云占位行不画，运行时的「下一个」也越过它（#10665）；按原始行数数，走到本页末尾时会多走一步才回到首条。`first_source` 是其中第一行的来源，没有这样的行时是 -1；没写来源的行照常数进去。
struct JapaneseSpaceRows {
  std::size_t count = 0;
  int first_source = -1;
};
// 按页的类型取模板，与 `visible_page_candidate` 一样不让这个头引入 nlohmann。
template <typename Page>
JapaneseSpaceRows japanese_space_rows(const Page &page) {
  JapaneseSpaceRows rows;
  for (const auto &candidate : page) {
    const auto found = candidate.find("source");
    const int source = found != candidate.end() && found->is_number_integer() ? found->template get<int>() : -1;
    if (source >= 0 && !candidate_source_is_visible(static_cast<unsigned>(source)))
      continue;
    if (rows.count++ == 0)
      rows.first_source = source;
  }
  return rows;
}
} // namespace msime::windows
