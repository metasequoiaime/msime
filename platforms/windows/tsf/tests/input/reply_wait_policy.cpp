#include <cstddef>
#include <initializer_list>
#include <stdexcept>

#include "../../IPC/ReplyWaitPolicy.h"
#include "../../../../../shared/contracts/quick_phrase_cursor_pipe.h"

namespace
{
void require(bool condition, const char *message)
{
    if (!condition) throw std::runtime_error(message);
}
} // namespace

int main()
{
    // 现场最高一次回包耗时为 61ms；低于 100ms 会继续把这类正常回包当成断线。
    require(msime::windows::tsf::kServerReplyWaitTimeoutMs >= 100,
            "server reply budget must cover the observed 61ms candidate response");
    // 软等待没等到回包（#10826）：按键已送到 Server，这个帧既不是要上屏的文字、也不是 Server 的任何决定、也不是断线。以前它是 Normal 加空文字，数字选词把它当成「Server 整段上屏了空文字」，选词失败后整段读音被重放。
    const auto missed = msime::windows::tsf::SoftReplyMiss(42);
    require(missed.request_id == 42, "the missed frame names its request");
    require(missed.candidate_string[0] == 0, "the missed frame carries no text");
    for (const auto decided : {FanyImeReplyType::Normal, FanyImeReplyType::OutofRange, FanyImeReplyType::NeedToCreateWord,
                               FanyImeReplyType::Preedit, FanyImeReplyType::NavigationIgnored,
                               FanyImeReplyType::CommitExactText, FanyImeReplyType::UiLessComposition,
                               FanyImeReplyType::TransportUnavailable})
        require(missed.msg_type != decided, "a missed reply is no Server reply type and no transport failure");
    require(!FanyImeQuickPhraseCursorPipe::SuffixUnits(missed.msg_type, false) &&
                !FanyImeQuickPhraseCursorPipe::MessageType(missed.msg_type, false),
            "a missed reply never decodes as a cursor commit or a valid wire frame");
    return 0;
}
