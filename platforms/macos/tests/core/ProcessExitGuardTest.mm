#import "../../src/core/ProcessExitGuard.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <initializer_list>
#include <signal.h>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

// imklaunchagent 只对退出状态 9 的死亡不计数（ProcessExitGuard.h）。这里起真的子进程，装上守卫后按输入法会遇到的各种方式结束，检查每一种都是被 SIGKILL 结束的；不装守卫的对照组确认这些方式本来不是。

extern char **environ;

namespace {
// 崩溃记录器的接法：后装的处理函数把守卫当作前任，做完自己的事再调用它（CrashRecorder.cpp 的 crash_signal）。
struct sigaction gPrevious;
volatile sig_atomic_t gRecorded = 0;

void Recorder(int number) {
    gRecorded = 1;
    if (gPrevious.sa_handler != SIG_DFL && gPrevious.sa_handler != SIG_IGN && gPrevious.sa_handler) gPrevious.sa_handler(number);
    // 前任没有结束进程时，把记录器看到过信号这件事留在退出码里，父进程据此判失败。
    _exit(gRecorded ? 42 : 43);
}

int run_child(const char *mode, bool guarded) {
    if (guarded) MSIMEInstallProcessExitGuard();
    if (!std::strcmp(mode, "return")) return 0;
    if (!std::strcmp(mode, "exit")) std::exit(0);
    if (!std::strcmp(mode, "failure")) std::exit(1);
    if (!std::strcmp(mode, "abort")) std::abort();
    if (!std::strcmp(mode, "segv")) {
        volatile int *address = nullptr;
        *address = 1;
    }
    if (!std::strcmp(mode, "trap")) __builtin_trap();
    if (!std::strcmp(mode, "recorder")) {
        struct sigaction action = {};
        action.sa_handler = Recorder;
        sigemptyset(&action.sa_mask);
        sigaction(SIGABRT, &action, &gPrevious);
        std::abort();
    }
    return 3;
}

bool spawn(const char *executable, const char *mode, bool guarded, int *status) {
    char *arguments[] = {const_cast<char *>(executable), const_cast<char *>("--child"), const_cast<char *>(mode),
                         const_cast<char *>(guarded ? "guarded" : "plain"), nullptr};
    pid_t child = 0;
    if (posix_spawn(&child, executable, nullptr, nullptr, arguments, environ) != 0) return false;
    return waitpid(child, status, 0) == child;
}

bool killed(int status) { return WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL; }
} // namespace

int main(int argc, char **argv) {
    if (argc == 4 && !std::strcmp(argv[1], "--child")) return run_child(argv[2], !std::strcmp(argv[3], "guarded"));
    bool ok = true;
    for (const char *mode : {"return", "exit", "failure", "abort", "segv", "trap", "recorder"}) {
        int status = 0;
        if (!spawn(argv[0], mode, true, &status) || !killed(status)) {
            std::fprintf(stderr, "process-exit-guard: a guarded process ending by %s did not die of SIGKILL (status %d)\n", mode, status);
            ok = false;
        }
        if (!spawn(argv[0], mode, false, &status) || killed(status)) {
            std::fprintf(stderr, "process-exit-guard: the unguarded control ending by %s did not behave as a control (status %d)\n", mode, status);
            ok = false;
        }
    }
    return ok ? 0 : 1;
}
