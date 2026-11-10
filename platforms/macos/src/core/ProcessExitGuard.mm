#import "ProcessExitGuard.h"

#include <signal.h>
#include <stdlib.h>
#include <unistd.h>

namespace {
// 默认动作是结束进程、而且输入法里可能真的出现的信号。前六个与崩溃记录器接管的一致（CrashRecorder.cpp 的 crash_signals）。SIGTERM 不在这里：它由 UsageReporting.mm 的调度源接管，先结束会话再退出。
constexpr int kFatalSignals[] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT, SIGTRAP, SIGSYS, SIGPIPE};

void FatalSignal(int) { MSIMEExitByKill(); }

void ExitHandler(void) { MSIMEExitByKill(); }
} // namespace

void MSIMEExitByKill(void)
{
    // kill 和 pause 都是异步信号安全的。SIGKILL 不能被屏蔽或接管，但发给整个进程的信号不一定在 kill 返回前落到本线程；这里只等它生效，不能接着调用 _exit，那样会抢在 SIGKILL 之前以普通退出码结束，系统照样计一次死亡。
    kill(getpid(), SIGKILL);
    for (;;) pause();
}

void MSIMEInstallProcessExitGuard(void)
{
    for (int number : kFatalSignals) {
        struct sigaction action = {};
        action.sa_handler = FatalSignal;
        // 栈溢出时原栈已经用不了；崩溃记录器装好备用栈之后这里也跑在备用栈上。
        action.sa_flags = SA_ONSTACK;
        sigemptyset(&action.sa_mask);
        sigaction(number, &action, nullptr);
    }
    // `[NSApp terminate:]`、main 返回和 `exit()` 都经过这里。登记得早，所以在别的 atexit 回调之后才运行。
    atexit(ExitHandler);
}
