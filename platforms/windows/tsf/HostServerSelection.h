#pragma once
#include "EngineSessionAdapter.h"
#include "../common/CandidateSources.h"
#include "../../../shared/contracts/windows_ipc.h"
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace msime::tsf {
// Server 对一次键盘选词（数字、二三候选键、空格）的回复：消息类型和 candidate_string 换成 UTF-8 的文字。
struct ServerSelectionReply {
    unsigned type = FanyImeReplyType::TransportUnavailable;
    std::string text;
};

// 这种回复是不是 Server 已经替这个键做了决定：选中了一项（Normal 整段上屏、NeedToCreateWord 只选一段），或者什么都没选（NavigationIgnored、OutofRange）。是的话宿主会话只能照着它选，不能再按自己那一页的位置选——候选窗是 Server 画的，用户数的编号数在它那一页上，而宿主会话是另一个引擎会话，鼠标滚轮和翻页箭头只翻 Server 那一页，两边的顺序也会各自学习后分开，同一个位置上常常是另一个词。读不到回复（超时、管道断开）时不是，调用方才退回本地高亮项。
inline bool ServerReplyDecidesSelection(unsigned type) {
    return type == FanyImeReplyType::Normal || type == FanyImeReplyType::NeedToCreateWord ||
           type == FanyImeReplyType::NavigationIgnored || type == FanyImeReplyType::OutofRange;
}

// Server 这一次选中的那一段候选的文字（UTF-8），宿主会话要选的就是它；什么都没选或认不出时为空。
//
// Normal 的文字是整次上屏：组字里已经暂存的词段在前（Runtime 的 phrase_prefix），这次选的候选接在后面。NeedToCreateWord 是「剩下的原文\t已选的全部词段\t显示的预编辑」，第二段同样是暂存词段加上这次选的。两种都要减掉宿主会话里已经暂存的那段，剩下的才是候选自己的文字；以前直接拿整段去宿主会话的候选里找（空格的 NeedToCreateWord 甚至拿整条带制表符的载荷去找），选过一段之后就再也对不上。宿主会话暂存的词段不是这段文字的开头时，两个会话已经分叉，原样返回，交给按文字查找去碰运气，找不到就拒绝。
inline std::string ServerSelectedPiece(const ServerSelectionReply &reply, const std::string &hostPhrasePrefix) {
    std::string selected;
    if (reply.type == FanyImeReplyType::Normal) {
        selected = reply.text;
    } else if (reply.type == FanyImeReplyType::NeedToCreateWord) {
        const auto first = reply.text.find('\t');
        if (first == std::string::npos) return {};
        const auto second = reply.text.find('\t', first + 1);
        selected = reply.text.substr(first + 1, second == std::string::npos ? std::string::npos : second - first - 1);
    } else {
        return {};
    }
    if (!hostPhrasePrefix.empty() && selected.size() > hostPhrasePrefix.size() &&
        selected.compare(0, hostPhrasePrefix.size(), hostPhrasePrefix) == 0)
        return selected.substr(hostPhrasePrefix.size());
    // 一段也没多出来（NeedToCreateWord 里暂存的词段没变）就是这次什么都没选。
    if (!hostPhrasePrefix.empty() && selected == hostPhrasePrefix) return {};
    return selected;
}

enum class HostSelectionOutcome {
    // 在宿主会话里选中了；`raw` 是选词的回复。
    Selected,
    // 文字落在占位席位上（云的「☁」）：它占着一格却选不中，引擎对它什么都不做但算处理过，按键要吃掉、组字原样留着。
    PendingSeat,
    // 找不到、不止一个同形的、或选择失败：什么都没选。
    Refused,
};

// 在宿主会话里选中文字是 `target` 的候选，用宿主会话自己的身份（代次、下标），不套用 Server 的。先在当前页上找唯一的一个；当前页上没有就到这一代的完整候选列表里找唯一的一个，按全局下标选。后一步是必需的：Server 那一页可能已经被滚轮翻走，或者两边的顺序已经分开，用户选中的词在宿主会话里常常不在同一页上。同形的不止一个时不猜。取完整列表可能让引擎放出扣着的尾部、代次前进，所以用列表自己的代次。
template <class Host>
HostSelectionOutcome SelectHostCandidateByText(Host &host, const EngineView &view, const std::string &target,
                                               std::string *raw, std::string *error) {
    if (target.empty()) return HostSelectionOutcome::Refused;
    const EngineCandidate *found = nullptr;
    for (const auto &candidate : view.candidates) {
        if (candidate.text != target) continue;
        if (found) return HostSelectionOutcome::Refused;
        found = &candidate;
    }
    if (found) {
        if (msime::windows::candidate_source_is_pending_seat(found->source)) return HostSelectionOutcome::PendingSeat;
        return host.select(view.generation, found->index, raw, error) ? HostSelectionOutcome::Selected
                                                                      : HostSelectionOutcome::Refused;
    }
    try {
        if (!host.all_candidates(raw, error)) return HostSelectionOutcome::Refused;
        const auto all = nlohmann::json::parse(*raw).at("value");
        if (all.value("session", view.session) != view.session) return HostSelectionOutcome::Refused;
        bool matched = false;
        std::size_t index = 0;
        unsigned source = 0;
        for (const auto &candidate : all.at("candidates")) {
            if (candidate.at("text").get<std::string>() != target) continue;
            if (matched) return HostSelectionOutcome::Refused;
            matched = true;
            index = candidate.at("id").at("index").get<std::size_t>();
            source = candidate.value("source", 0u);
        }
        if (!matched) return HostSelectionOutcome::Refused;
        if (msime::windows::candidate_source_is_pending_seat(source)) return HostSelectionOutcome::PendingSeat;
        return host.select_any(all.at("generation").get<uint64_t>(), index, raw, error)
                   ? HostSelectionOutcome::Selected
                   : HostSelectionOutcome::Refused;
    } catch (...) {
        return HostSelectionOutcome::Refused;
    }
}
} // namespace msime::tsf
