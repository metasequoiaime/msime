#pragma once

#include "../../../../shared/input/JapaneseConversion.h"

#include <nlohmann/json.hpp>

namespace msime::linux_host {

// 日语空格「変換」的状态机放在 shared/input/JapaneseConversion.h，Linux 的两个前端和 Windows Server 共用；这里保留原来的名字，调用方不必改。
using JapaneseConversion = msime::input::JapaneseConversion;
using msime::input::kCandidateSourceCloudPending;
using msime::input::kCandidateSourceFallback;

// 空格数的、回车按 `index()` 选的行：这一页去掉云占位行。Linux 照旧把「☁」画出来，但运行时的「下一个」越过它（#10665），两个前端都必须按这份数，不能按原始的 `candidates`。
inline nlohmann::json japanese_conversion_rows(const nlohmann::json &candidates)
{
    auto rows = nlohmann::json::array();
    if (!candidates.is_array())
        return rows;
    for (const auto &candidate : candidates) {
        const auto source = candidate.is_object() ? candidate.find("source") : candidate.end();
        const bool placeholder = source != candidate.end() && source->is_number_integer() &&
                                 source->get<int>() == kCandidateSourceCloudPending;
        if (!placeholder)
            rows.push_back(candidate);
    }
    return rows;
}

} // namespace msime::linux_host
