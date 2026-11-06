#pragma once

#include <cstddef>

namespace msime::windows::tsf
{
// 必须确认的回包路径至少留出两倍于现场最高 61ms 回包时间的余量；软回包路径仍使用调用方自己的预算。
inline constexpr std::size_t kServerReplyWaitTimeoutMs = 100;

// 必须确认的回包（选词、带候选的标点提交、联想选择）超过第一级预算后，只要管道没有报错就继续等到这个上限才判定传输失败。管道真断时重叠读会立刻以失败返回，不会走到这里；超时只说明 Server 还连着但没来得及回，CPU 被占满时就是这样（#10826）。把慢回包当断线会拆管道、重连、把整串组字连同选词键一起重放，代价远大于多等这几百毫秒。
inline constexpr std::size_t kServerReplyStallBudgetMs = 1000;

// 一次回包等待的总预算：软回包只等第一级，等不到由调用方回落到本地状态；必须确认的回包等到第二级上限。
inline constexpr std::size_t ServerReplyWaitBudgetMs(bool abortTransportOnTimeout) noexcept
{
    return abortTransportOnTimeout ? kServerReplyStallBudgetMs : kServerReplyWaitTimeoutMs;
}

// 回包在第一级预算之后才到，记成慢回包而不是普通回包，现场日志能分清「Server 慢」和「Server 正常」。
inline constexpr bool IsSlowServerReply(double elapsedMs) noexcept
{
    return elapsedMs > static_cast<double>(kServerReplyWaitTimeoutMs);
}
} // namespace msime::windows::tsf
