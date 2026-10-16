#pragma once
#include "../../../shared/contracts/candidate_continuation_pipe.h"
#include <nlohmann/json.hpp>

namespace msime::windows {
// 云响应最多 256 KiB，ASCII JSON 最坏六倍展开；仍有明确分帧与重组上限。
constexpr std::size_t HostOnlinePayloadLimit = 2 * 1024 * 1024;
using HostOnlineAssembler = FanyImeCandidateContinuationPipe::BasicAssembler<HostOnlinePayloadLimit>;
inline nlohmann::json online_semantics(nlohmann::json query) {
    query.erase("session_id"); query.erase("generation"); query.erase("ai_context");
    return query;
}
// 只在 TIP 所属线程使用。保存发起按键执行后的原始查询，绝不拿回调到达时的新查询重贴身份。
class HostOnlineCandidateSync {
public:
    template<class Host> void capture(Host &host, uint64_t request) {
        request_ = 0; query_.clear();
        if (!request || request == FANY_IME_NO_REQUEST_ID || !host.valid()) return;
        request_ = request;
        std::string raw, error;
        if (!host.online_query(&raw, &error)) return;
        try {
            const auto query = nlohmann::json::parse(raw).at("value");
            if (query.is_object()) query_ = query.dump();
        } catch (...) {}
    }
    template<class Host> bool apply(Host &host, const std::string &payload) {
        try {
            if (!host.valid() || payload.size() > HostOnlinePayloadLimit) return false;
            const auto value = nlohmann::json::parse(payload);
            if (value.at("version") != 1 || !request_ || value.at("request") != request_) return false;
            const auto operation = value.at("operation").get<unsigned>();
            std::string raw, error;
            if (operation == 2) {
                if (query_.empty() || !host.online_query(&raw, &error) ||
                    nlohmann::json::parse(raw).at("value") != nlohmann::json::parse(query_)) return false;
                if (!host.rerank_settled(&raw, &error)) return false;
                return nlohmann::json::parse(raw).at("value").value("moved", false);
            }
            if (operation > 1 || query_.empty() || online_semantics(nlohmann::json::parse(query_)) !=
                                                   online_semantics(value.at("query"))) return false;
            if (operation == 0) {
                const auto body = value.at("body").get<std::string>();
                if (body.empty() || body.size() > 262144 || !host.apply_cloud(query_, body, &raw, &error)) return false;
            } else {
                const auto candidates = value.at("candidates").dump();
                if (candidates.size() > 16384 || !host.apply_ai(query_, candidates, &raw, &error)) return false;
                if (!nlohmann::json::parse(raw).at("value").value("applied", false) &&
                    !host.resolve_ai(query_, &raw, &error)) return false;
            }
            return nlohmann::json::parse(raw).at("value").value("applied", false);
        } catch (...) { return false; }
    }
private:
    uint64_t request_ = 0;
    std::string query_;
};
} // namespace msime::windows
