#include <cstddef>
#include <stdexcept>

#include "../../IPC/ReplyWaitPolicy.h"

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
    return 0;
}
