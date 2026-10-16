#pragma once
#include "FocusGate.h"
#include "ReplyComposer.h"
#include "ipc_negotiation.h"
#include "../../common/HostOnlineCandidateSync.h"

namespace msime::windows {
// 输入队列所有：只向实际请求本地候选的 UI-less 租约回送原始操作。
class HostCandidateRelay {
public:
    void delivered(const FocusLease &lease, const PendingReply &reply, const FanyImeNamedpipeData &packet) {
        lease_.reset(); query_.reset();
        if (!(packet.modifiers_down & FanyImePipeFlags::UiLess) ||
            (packet.modifiers_down & FanyImePipeFlags::ServerUiLessCandidates) ||
            !(lease.transport.capabilities & FanyImeProtocol::HostOnlineCandidates) ||
            reply.source.transition.at("view").at("editing_text").get<std::string>().empty()) return;
        lease_ = lease; request_ = reply.source.request_id; query_ = reply.online_query;
    }
    std::optional<std::string> update(const FocusLease &lease, unsigned operation,
                                     const std::string &query, const nlohmann::json &result) const {
        if (!lease_ || lease_->epoch != lease.epoch || lease_->token != lease.token ||
            !same_ticket(lease_->transport, lease.transport) || !request_ ||
            (operation != 2 && (!query_ || *query_ != query))) return std::nullopt;
        nlohmann::json value{{"version", 1}, {"request", request_}, {"operation", operation}};
        if (operation != 2) value["query"] = nlohmann::json::parse(query);
        if (operation == 0) value["body"] = result;
        if (operation == 1) value["candidates"] = result;
        return value.dump(-1, ' ', true);
    }
private:
    std::optional<FocusLease> lease_;
    std::optional<std::string> query_;
    uint64_t request_ = 0;
};
} // namespace msime::windows
