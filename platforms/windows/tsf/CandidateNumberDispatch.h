#pragma once

#include "../../../shared/contracts/windows_ipc.h"

namespace msime::tsf {

// 本地槽位不足时仍须消费已发送的选词请求，不能把 Server 的回复留在管道中。
template <class Result, class SelectSlot, class Convert, class FollowServer>
Result DispatchCandidateNumber(int slot, bool engine_page, bool reply_expected,
                               Result no_selection, SelectSlot select_slot,
                               Convert convert, FollowServer follow_server) {
    if (slot >= 0 && select_slot(slot)) return convert();
    if (engine_page && reply_expected) return follow_server();
    return no_selection;
}

// Server 确认未选中候选时，完成这次按键，保持当前组字，不触发失败重放。
template <class Result, class Select>
Result CompleteServerCandidateSelection(unsigned reply_type, Result handled, Select select) {
    if (reply_type == FanyImeReplyType::NavigationIgnored || reply_type == FanyImeReplyType::OutofRange)
        return handled;
    return select();
}

} // namespace msime::tsf
