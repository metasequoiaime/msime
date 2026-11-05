// 回包等待预算的两级规则，以及接线：Ipc.cpp 按调用方是否必须确认来取预算，宿主会话里的数字选词等到第二级上限。读回包的函数和选词路径要整个 TIP 才能构建，所以接线这里核对源码。从仓库根目录运行，或传入 TSF 源码目录。
#include <algorithm>
#include <cstddef>
#include <fstream>
#include <sstream>
#include <stdexcept>
#include <string>

#include "../../IPC/ReplyWaitPolicy.h"

namespace
{
void require(bool condition, const char *message)
{
    if (!condition) throw std::runtime_error(message);
}

std::string read(const std::string &path)
{
    std::ifstream file(path, std::ios::binary);
    if (!file) throw std::runtime_error("cannot open " + path);
    std::ostringstream text;
    text << file.rdbuf();
    // Git for Windows 默认 autocrlf 检出成 CRLF，读进来时去掉回车，换行一律按 LF 比较。
    std::string content = text.str();
    content.erase(std::remove(content.begin(), content.end(), '\r'), content.end());
    return content;
}
} // namespace

int main(int argc, char **argv)
{
    using namespace msime::windows::tsf;

    // 现场最高一次回包耗时为 61ms；低于 100ms 会继续把这类正常回包当成断线。
    require(kServerReplyWaitTimeoutMs >= 100, "server reply budget must cover the observed 61ms candidate response");

    // #10826：CPU 被占满时选词回包超过 100ms，管道还连着却被拆掉重连，整串组字连同选词键被重放。必须确认的回包要等到第二级上限，软回包不变。
    require(ServerReplyWaitBudgetMs(false) == kServerReplyWaitTimeoutMs,
            "soft reply waits keep the first-level budget");
    require(ServerReplyWaitBudgetMs(true) == kServerReplyStallBudgetMs,
            "confirmed reply waits use the stall budget");
    require(kServerReplyStallBudgetMs >= 5 * kServerReplyWaitTimeoutMs,
            "stall budget must tolerate a CPU-starved Server well past the first-level budget");
    require(kServerReplyStallBudgetMs <= 1000, "stall budget must stay bounded so a hung Server still resets");

    require(!IsSlowServerReply(61.0), "a field-normal reply is not slow");
    require(!IsSlowServerReply(static_cast<double>(kServerReplyWaitTimeoutMs)), "the first-level edge is not slow");
    require(IsSlowServerReply(static_cast<double>(kServerReplyWaitTimeoutMs) + 1.0),
            "a reply past the first-level budget is slow");

    const std::string root = argc > 1 ? argv[1] : "platforms/windows/tsf";
    const std::string ipc = read(root + "/IPC/Ipc.cpp");
    require(ipc.find("ServerReplyWaitBudgetMs(abortTransportOnTimeout)") != std::string::npos,
            "TryReadDataFromServerPipeWithTimeout must pick its budget from abortTransportOnTimeout");
    require(ipc.find("const ULONGLONG deadline = GetTickCount64() + waitBudgetMs;") != std::string::npos,
            "the reply deadline must come from the caller's budget");
    require(ipc.find("IsSlowServerReply(elapsedMs)") != std::string::npos,
            "late confirmed replies must be logged as reply-wait-slow");

    // 宿主会话拥有组字时数字选词读的是软回包，读不到就什么也不上屏；回复一定会来，要等到第二级上限，仍不拆管道。
    const std::string presenter = read(root + "/UI/CandidateListUIPresenter.cpp");
    const auto worker = presenter.find("HRESULT CMetasequoiaIME::_HandleCandidateWorker(");
    require(worker != std::string::npos, "_HandleCandidateWorker must exist");
    const auto workerEnd = presenter.find("\n}\n", worker);
    const auto hostRead = presenter.find(
        "TryReadDataFromServerPipeWithTimeout(requestId, /*abortTransportOnTimeout=*/false, "
        "msime::windows::tsf::kServerReplyStallBudgetMs)",
        worker);
    require(hostRead != std::string::npos && hostRead < workerEnd,
            "the host digit selection must wait for the Server's reply up to the stall budget without tearing down");
    return 0;
}
