#pragma once
#include "EngineSessionAdapter.h"
#include "../common/CandidateContinuation.h"
#include "../../../shared/contracts/candidate_continuation_pipe.h"

namespace msime::tsf {
// 在 TIP 的编辑会话里执行；候选必须在本地当前列表中唯一匹配，不能套用 Server 的身份。
template<class Host>
bool ContinueHostCandidate(Host &host, const std::string &payload) {
    using namespace msime::windows;
    try {
        if (!host.valid() || payload.size() > FanyImeCandidateContinuationPipe::MaxPayloadChars) return false;
        const auto request = nlohmann::json::parse(payload);
        if (request.at("version") != 1) return false;
        const auto target = request.at("candidate").get<std::string>();
        if (target.empty() || target.find('\0') != std::string::npos) return false;
        std::string raw, error;
        if (!host.view(&raw, &error)) return false;
        const auto current = nlohmann::json::parse(raw).at("value");
        if (candidate_continuation_state(current) != request.at("before")) return false;
        EngineResult parsed;
        if (!EngineSessionAdapter::parse_result(raw, &parsed, &error)) return false;
        const EngineCandidate *found = nullptr;
        for (const auto &candidate : parsed.view.candidates) {
            if (candidate.text != target) continue;
            if (found) return false;
            found = &candidate;
        }
        if (!found || !host.select(parsed.view.generation, found->index, &raw, &error)) return false;
        const auto selected = nlohmann::json::parse(raw).at("value");
        return selected.at("handled").get<bool>() && selected.at("commit").is_null() &&
               candidate_continuation_state(selected.at("view")) == request.at("after");
    } catch (...) { return false; }
}
} // namespace msime::tsf
