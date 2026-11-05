#pragma once

#include <cstddef>
#include <cstdint>

#include "../../../../shared/contracts/windows_ipc.h"

namespace msime::windows::tsf
{
// 必须确认的回包路径至少留出两倍于现场最高 61ms 回包时间的余量；软回包路径仍使用调用方自己的预算。
inline constexpr std::size_t kServerReplyWaitTimeoutMs = 100;

// 软等待（abortTransportOnTimeout=false）没等到这个请求的回包时交给调用方的帧。按键已经送到 Server，它做过或稍后会做这个键的决定，只是这一次读不到；调用方按「读不到回复」走本地退路，不能把它当成 Server 的决定。
inline FanyImeNamedpipeDataToTsf SoftReplyMiss(std::uint64_t requestId) noexcept
{
    FanyImeNamedpipeDataToTsf frame{};
    frame.msg_type = FanyImeReplyType::ReplyMissed;
    frame.request_id = requestId;
    return frame;
}
} // namespace msime::windows::tsf
