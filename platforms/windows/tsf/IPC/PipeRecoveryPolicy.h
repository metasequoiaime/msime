#pragma once

#include <cstdint>

#include "../../../../shared/contracts/windows_ipc.h"

namespace msime::windows::tsf
{
// 反向端点（reply / worker）重新握手后，本地 Main 必须跟着重开。Server 每注册一个反向端点都会先
// 清掉它记住的 Main（PipeRegistry 的 register_reverse 在发出 pipe_ready 之后 clear(Main)），
// 旧 Main 句柄已经没有对端在收：继续往它写 ClientActivated，回执永远不会回来，上层只看到
// 「激活发出去了却没确认」，超时后又一次重连，循环停不下来。
inline constexpr bool ReopenMainAfterPipeOpen(std::uint32_t role) noexcept
{
    return role != FanyImePipeRole::Main;
}

// 一次焦点激活等回执的结论。回执到了才放行；会话正在重置、或者等满预算仍然没有回执，都算
// 传输失败——必须换一代激活令牌并重连，而不是把「没确认」当成「已就绪」继续发包。
enum class FocusEchoOutcome
{
    Ready,
    ResetTransport,
};

inline constexpr FocusEchoOutcome FocusEchoOutcomeFor(bool acknowledged, bool sessionResetPending) noexcept
{
    if (sessionResetPending)
    {
        return FocusEchoOutcome::ResetTransport;
    }
    return acknowledged ? FocusEchoOutcome::Ready : FocusEchoOutcome::ResetTransport;
}
} // namespace msime::windows::tsf
