#pragma once

#include <cstddef>

namespace msime::windows::tsf
{
// 必须确认的回包路径至少留出两倍于现场最高 61ms 回包时间的余量；软回包路径仍使用调用方自己的预算。
inline constexpr std::size_t kServerReplyWaitTimeoutMs = 100;
}
