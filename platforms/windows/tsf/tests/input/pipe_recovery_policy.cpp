#include <stdexcept>

#include "../../IPC/PipeRecoveryPolicy.h"

namespace
{
void require(bool condition, const char *message)
{
    if (!condition) throw std::runtime_error(message);
}
} // namespace

int main()
{
    namespace tsf = msime::windows::tsf;
    // 反向端点重开会让 Server 清掉它记住的 Main，所以只有反向端点需要连带重开 Main。
    require(!tsf::ReopenMainAfterPipeOpen(FanyImePipeRole::Main),
            "开 Main 本身不能反过来把 Main 关掉，否则每一轮都会重新握手");
    require(tsf::ReopenMainAfterPipeOpen(FanyImePipeRole::ToTsf),
            "reply 端点重开时本地 Main 已经没有对端在收，必须重开");
    require(tsf::ReopenMainAfterPipeOpen(FanyImePipeRole::ToTsfWorkerThread),
            "worker 端点重开时本地 Main 已经没有对端在收，必须重开");

    // 回执到了才放行；没到或会话正在重置，都是传输失败，要换一代令牌重连。
    require(tsf::FocusEchoOutcomeFor(true, false) == tsf::FocusEchoOutcome::Ready,
            "收到回执后应放行这一代会话");
    require(tsf::FocusEchoOutcomeFor(false, false) == tsf::FocusEchoOutcome::ResetTransport,
            "等满预算没有回执就是传输失败，不能当成已就绪");
    require(tsf::FocusEchoOutcomeFor(true, true) == tsf::FocusEchoOutcome::ResetTransport,
            "会话正在重置时即使看到旧回执也不能放行");
    require(tsf::FocusEchoOutcomeFor(false, true) == tsf::FocusEchoOutcome::ResetTransport,
            "会话正在重置时没有回执同样是传输失败");
    return 0;
}
