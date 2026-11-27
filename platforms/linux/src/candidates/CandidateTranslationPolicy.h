#pragma once

#include "../../../../shared/input/GlossSenses.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace msime::linux_host {

// Candidate sources whose text is display-only and must never be sent to a
// user-owned translation service. The shared query carries this identity as
// `online_gloss: false`, but Linux's provider socket intentionally accepts a
// text-only candidate list, so hosts must apply the source gate before
// stripping the metadata.
inline bool candidate_translation_source_allowed(std::uint64_t source) {
  return source != 6 && source != 7 && source != 13 && source != 14 && source != 18;
}

// The dictionary joins multiple senses with either an ASCII or a full-width semicolon; IBus and Fcitx5 present them as the same secondary translation choices. The rule is shared with every other host - see shared/input/GlossSenses.h.
inline std::vector<std::string>
split_translation_gloss(std::string_view gloss) {
  return msime::input::gloss_senses(gloss);
}

// Folds the user's own translator's answers into a non-English offline dictionary's glosses for the same page.
//
// The dictionary answers at once and is shown first; the provider is then asked about every candidate, not only the dictionary's misses as on the English path, because the shared translation query ranks the user's own translator above an installed dictionary. So an online gloss replaces the dictionary's for the same text in place, a text only the provider answered is appended, and the dictionary keeps whatever the provider left.
inline void prefer_online_glosses(
    std::vector<std::pair<std::string, std::string>> &glosses,
    const std::vector<std::pair<std::string, std::string>> &online) {
  glosses.reserve(glosses.size() + online.size());
  for (const auto &entry : online) {
    if (entry.first.empty() || entry.second.empty())
      continue;
    const auto existing = std::find_if(
        glosses.begin(), glosses.end(),
        [&](const auto &item) { return item.first == entry.first; });
    if (existing == glosses.end())
      glosses.push_back(entry);
    else
      existing->second = entry.second;
  }
}

// Whether a translation query is the `/fy` command's rather than a candidate gloss request: the command mode's query is one sentence, the English after the trigger, and its answer becomes the command's first row instead of a gloss. It goes to the translation service the user chose whatever the gloss switches say, never to an offline dictionary and never to another service, and its target language is the one the query names.
inline bool command_translation_query(std::string_view local_mode, bool sentence) {
  return local_mode == "command" && sentence;
}

// 横排候选窗为还没到的译文预留几行，与 macOS 的 reservedGlossHeightForFont 相同：数配置的目标语言（主语言，以及与它不同的第二语言，只认受支持的七种），至少一行、至多两行。
inline std::size_t translation_reserved_lines(std::string_view primary, std::string_view secondary) {
  constexpr std::array<std::string_view, 7> supported{"en", "fr", "ja", "es", "ru", "de", "ko"};
  const auto known = [&](std::string_view language) {
    return std::find(supported.begin(), supported.end(), language) != supported.end();
  };
  std::size_t count = known(primary) ? 1 : 0;
  if (known(secondary) && secondary != primary) ++count;
  return std::clamp<std::size_t>(count, 1, 2);
}

// classicui 只画得了按行数算高的候选行，而横排候选条的高度取整页最高的那一行、行内三段又是垂直居中：高亮行一旦变成两行，整条候选条就跟着一行空白变高，那一格的序号也掉到候选字下面（#11821）。所以横排既不加占位行（占位行只在竖排加，横排返回 0），释义也不另起一行（classicui_gloss_inline 为真时接在同一行，与 IBus 查找表一致）；留高本身照旧交给原生画布（display.reserved_gloss_lines）。
inline int classicui_gloss_placeholder_lines(bool vertical, int reserved) {
  return vertical ? reserved : 0;
}

// 横排 classicui 把候选词翻译接在候选字后面同一行（「候选 · 释义」），竖排仍放在下一行。
inline bool classicui_gloss_inline(bool vertical) {
  return !vertical;
}

// 在线服务请求失败或没有返回任何译文时，当前查询仍应允许下一次刷新重试。
inline bool should_retry_translation_after_provider(bool requested, bool response_valid,
                                                    bool answered) {
  return requested && (!response_valid || !answered);
}

} // namespace msime::linux_host
