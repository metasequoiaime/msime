#pragma once
#include "../../../../shared/contracts/quick_phrase_cursor_pipe.h"
#include "CommitCandidateAndContinuePayload.h"
#include <utility>

namespace msime::tsf {
// Worker 的继续提交带消费计数，位置只属于去掉该前缀之后的真实正文。
inline std::optional<FanyImeQuickPhraseCursorPipe::CommitMetadata> DecodeWorkerCursorReply(
    std::uint32_t type, const wchar_t *payload, std::size_t capacity, bool negotiated)
{
    if (!FanyImeQuickPhraseCursorPipe::HasCursor(type))
        return FanyImeQuickPhraseCursorPipe::CommitMetadata{type, std::nullopt};
    const auto message = FanyImeQuickPhraseCursorPipe::MessageType(type, true);
    if (!negotiated || !message || !FanyImeQuickPhraseCursorPipe::ValidText(payload, capacity, 0))
        return std::nullopt;
    std::wstring text(payload);
    if (*message == FanyImeWorkerReplyType::CommitCandidateAndContinue) {
        std::size_t consumed = 0;
        std::wstring commit;
        if (!ParseCommitCandidateAndContinuePayload(text, consumed, commit)) return std::nullopt;
        text = std::move(commit);
    }
    return FanyImeQuickPhraseCursorPipe::Decode(type, text.c_str(), text.size() + 1, true, negotiated);
}
} // namespace msime::tsf
