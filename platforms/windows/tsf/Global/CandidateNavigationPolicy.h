#pragma once

#include "msime_client.h"
#include "windows_ipc.h"

namespace msime::windows::tsf
{
inline uint32_t CandidateNavigationCommand(uint32_t reply)
{
    switch (reply)
    {
    case FanyImeReplyType::MoveSelectionPrevious: return MSIME_PREVIOUS_CANDIDATE;
    case FanyImeReplyType::MoveSelectionNext: return MSIME_NEXT_CANDIDATE;
    case FanyImeReplyType::MovePagePrevious: return MSIME_PREVIOUS_PAGE;
    case FanyImeReplyType::MovePageNext: return MSIME_NEXT_PAGE;
    default: return 0;
    }
}
} // 命名空间 msime::windows::tsf
