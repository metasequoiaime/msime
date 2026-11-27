#pragma once
#include "FocusGate.h"
#include "ReplyCodec.h"
#include "ReplyComposer.h"
#include "ipc_negotiation.h"
#include "../../common/CandidateSources.h"

namespace msime::windows {
// 只在输入队列上使用。记住已确认送达的 UI-less
// 会话，后台结果不能投给别的焦点或下一轮组字。
class UiLessCandidateSync {
public:
  void delivered(const FocusLease &lease, const PendingReply &reply,
                 const FanyImeNamedpipeData &packet) {
    lease_.reset();
    if (!(packet.modifiers_down & FanyImePipeFlags::UiLess) ||
        !(packet.modifiers_down & FanyImePipeFlags::ServerUiLessCandidates) ||
        !(lease.transport.capabilities & FanyImeProtocol::UiLessCandidates))
      return;
    const auto &view = reply.source.transition.at("view");
    if (view.at("editing_text").get<std::string>().empty())
      return;
    lease_ = lease;
    session_ = view.at("session").get<uint64_t>();
    generation_ = view.at("generation").get<uint64_t>();
    prefix_ = reply.next_prefix;
  }
  std::optional<std::vector<uint8_t>> updated(const FocusLease &lease,
                                              const nlohmann::json &view) {
    if (!lease_ || lease_->epoch != lease.epoch ||
        lease_->token != lease.token ||
        !same_ticket(lease_->transport, lease.transport) ||
        view.at("session").get<uint64_t>() != session_ ||
        view.at("generation").get<uint64_t>() <= generation_ ||
        !view.at("focused").get<bool>() ||
        view.at("editing_text").get<std::string>().empty())
      return std::nullopt;
    std::vector<std::string> page;
    size_t highlighted = 0;
    bool found = false;
    for (const auto &item : view.at("candidates")) {
      // UI-less 宿主自己画候选页，页里只有文字，没有来源，所以这里必须先按候选窗同一条规则剔除云的占位席位（「☁」）：它在游戏宿主里被画成缺字方框，还占掉一个编号，而数字键经 Server 只数看得见的行——按 2 上屏的是宿主画出来的第 3 行。高亮停在占位席位上时不发这一帧，宿主的最旧一页保持不变，离开后自然恢复。
      if (!candidate_source_is_visible(item.value("source", 0u)))
        continue;
      if (item.at("highlighted").get<bool>()) {
        if (found)
          return std::nullopt;
        highlighted = page.size();
        found = true;
      }
      page.push_back(item.at("text").get<std::string>());
    }
    if (!page.empty() && !found)
      return std::nullopt;
    const auto generation = view.at("generation").get<uint64_t>();
    auto bytes =
        uiless_candidates_bytes(prefix_ + view.at("preedit").get<std::string>(),
                                page, highlighted, generation);
    // 有效的新代次却无法在线上表示时，用空帧让调用者断开；继续沿用旧页会导致选词错位。
    if (!bytes)
      return std::vector<uint8_t>{};
    generation_ = generation;
    return bytes;
  }

private:
  std::optional<FocusLease> lease_;
  uint64_t session_ = 0;
  uint64_t generation_ = 0;
  std::string prefix_;
};
} // namespace msime::windows
