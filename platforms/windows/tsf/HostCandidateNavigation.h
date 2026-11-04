#pragma once
#include "EngineSessionAdapter.h"
#include "../common/CandidateSources.h"
#include "msime_client.h"
#include <optional>
#include <string>

namespace msime::tsf {
// 回复里高亮的那一行；读不出回复或没有候选时为空。
inline std::optional<msime::windows::HighlightedSeat> HostHighlightedSeat(const std::string &raw, std::size_t *rows) {
    EngineResult result;
    std::string ignored;
    if (!EngineSessionAdapter::parse_result(raw, &result, &ignored)) return std::nullopt;
    if (rows) *rows = result.view.candidates.size();
    for (const auto &candidate : result.view.candidates)
        if (candidate.highlighted)
            return msime::windows::HighlightedSeat{candidate.index,
                                                   msime::windows::candidate_source_is_visible(candidate.source)};
    return std::nullopt;
}

// Server 的候选导航回复在 TIP 自己的宿主会话里照做一遍（#9754）：发同一条命令，再和 Server 一样把高亮从候选窗不画的占位席位上让开（`common/CandidateSources.h` 的 leave_hidden_seat）。Server 那一侧已经让过去了，这一侧只发一条命令的话高亮会停在席位上，两份会话就差了一格。`command` 为 0（NavigationIgnored）时什么都不发。`raw` 留着最后一次命令的回复；命令失败返回 false。
template <class Host>
bool NavigateHostCandidates(Host &host, uint32_t command, std::string *raw, std::string *error) {
    if (!command) return true;
    std::optional<std::size_t> before;
    if (host.view(raw, error))
        if (const auto seat = HostHighlightedSeat(*raw, nullptr)) before = seat->index;
    if (!host.command(command, raw, error)) return false;
    std::size_t rows = 0;
    (void)HostHighlightedSeat(*raw, &rows);
    return msime::windows::leave_hidden_seat(
        before, rows, [&] { return HostHighlightedSeat(*raw, nullptr); },
        [&](bool forward) {
            return host.command(forward ? MSIME_NEXT_CANDIDATE : MSIME_PREVIOUS_CANDIDATE, raw, error);
        });
}
} // namespace msime::tsf
