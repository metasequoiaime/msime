#pragma once
#include "windows_ipc.h"
#include <cstdint>
#include <string>
#include <string_view>

namespace Global
{
// 日语空格「変換」在 TIP 这边只记一件事：Server 已经开始了这段组字的转换。空格的开始、步进和回到首条都在 Server 的会话里执行（src/input/JapaneseSpacePolicy.h），TIP 从空格的回执类型得知转换开始了，回车时改为上屏 Server 高亮的候选，而不是自己宿主会话里的假名。`compositionEpoch` 是那时的组字代次，`reading` 是那时宿主会话的 editing_text：组字结束或读音改了，记号就作废，和 shared/input/JapaneseConversion.h 改读音即作废的规则一致。
struct JapaneseConversionMark
{
    uint64_t compositionEpoch = 0;
    std::string reading;
};

// 日语组字时空格的回执是导航类（开始转换回 NavigationIgnored，步进回 MoveSelectionNext）：空格没有上屏任何东西，组字要留着，转换从此算开始。其他方案的空格从不得到这样的回执，照原来的规则处理。
inline bool JapaneseSpaceReplyKeepsComposition(std::uint32_t replyType, bool japanese)
{
    if (!japanese)
        return false;
    switch (replyType)
    {
    case FanyImeReplyType::NavigationIgnored:
    case FanyImeReplyType::MoveSelectionPrevious:
    case FanyImeReplyType::MoveSelectionNext:
    case FanyImeReplyType::MovePagePrevious:
    case FanyImeReplyType::MovePageNext:
        return true;
    default:
        return false;
    }
}

// 这次裸回车上屏 Server 高亮的候选（转换已经开始，且还是同一段组字、同一段读音），否则照旧上屏假名。
inline bool JapaneseEnterCommitsCandidate(const JapaneseConversionMark &mark, uint64_t compositionEpoch,
                                          std::string_view reading, bool japanese, bool plainEnter)
{
    return japanese && plainEnter && mark.compositionEpoch != 0 && mark.compositionEpoch == compositionEpoch &&
           !reading.empty() && reading == mark.reading;
}
} // namespace Global
