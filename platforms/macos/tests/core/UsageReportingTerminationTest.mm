#import "../../src/core/UsageReporting.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <filesystem>
#include <signal.h>
#include <spawn.h>
#include <string>
#include <sys/wait.h>
#include <unistd.h>

// 输入法进程实际是被 SIGTERM 结束的（设置应用装好或更新输入法时的 `pkill`、注销、关机），不是 `NSApplicationWillTerminateNotification`。这里起一个真的子进程跑 `MSIMEUsageReportingStart`，再向它发 SIGTERM，检查会话照样经 `msime_client_telemetry_end` 结束，而且进程仍是被 SIGTERM 结束的。宿主函数换成桩，桩只往 `MSIME_USAGE_REPORTING_TEST_DIR` 下写标记文件。

extern char **environ;

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t);
extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t);
extern "C" void msime_client_string_free(char *value);

namespace {
constexpr const char *kDirectoryVariable = "MSIME_USAGE_REPORTING_TEST_DIR";
constexpr const char *kEndHangsVariable = "MSIME_USAGE_REPORTING_TEST_END_HANGS";

char *copy(const std::string &value) {
    char *result = static_cast<char *>(std::malloc(value.size() + 1));
    if (!result) std::abort();
    std::memcpy(result, value.c_str(), value.size() + 1);
    return result;
}

std::filesystem::path test_directory() {
    const char *value = std::getenv(kDirectoryVariable);
    if (!value) std::abort();
    return value;
}

void touch(const std::filesystem::path &path) {
    int fd = open(path.c_str(), O_WRONLY | O_CREAT, 0600);
    if (fd >= 0) close(fd);
}

bool wait_for(const std::filesystem::path &path, int milliseconds) {
    for (int waited = 0; waited < milliseconds; waited += 10) {
        if (std::filesystem::exists(path)) return true;
        usleep(10 * 1000);
    }
    return std::filesystem::exists(path);
}

bool fail(const char *message) {
    std::fprintf(stderr, "usage-reporting-termination: %s\n", message);
    return false;
}

// 子进程：开始上报，等 SIGTERM 处理就位（处置变成忽略），告诉父进程已就绪，然后等着被结束。
int run_child() {
    @autoreleasepool {
        MSIMEUsageReportingStart([NSString stringWithUTF8String:test_directory().c_str()]);
        for (int waited = 0; waited < 2000; waited += 10) {
            struct sigaction current = {};
            sigaction(SIGTERM, nullptr, &current);
            if (current.sa_handler == SIG_IGN) break;
            usleep(10 * 1000);
        }
        touch(test_directory() / "ready");
        sleep(30);
    }
    return 3;
}

struct ChildExit {
    bool exited = false;
    int status = 0;
    int elapsed_milliseconds = 0;
};

// 起一个子进程，就绪后向它发 SIGTERM，最多等 timeout 毫秒让它结束；超时就 SIGKILL 并报告没有结束。
ChildExit terminate_child(const char *executable, const std::filesystem::path &directory, bool end_hangs, int timeout) {
    std::filesystem::remove_all(directory);
    std::filesystem::create_directories(directory);
    setenv(kDirectoryVariable, directory.c_str(), 1);
    if (end_hangs) setenv(kEndHangsVariable, "1", 1);
    else unsetenv(kEndHangsVariable);
    char *arguments[] = {const_cast<char *>(executable), const_cast<char *>("--child"), nullptr};
    pid_t child = 0;
    ChildExit result;
    if (posix_spawn(&child, executable, nullptr, nullptr, arguments, environ) != 0) return result;
    if (!wait_for(directory / "ready", 10000)) {
        kill(child, SIGKILL);
        waitpid(child, nullptr, 0);
        return result;
    }
    kill(child, SIGTERM);
    for (; result.elapsed_milliseconds < timeout; result.elapsed_milliseconds += 10) {
        pid_t done = waitpid(child, &result.status, WNOHANG);
        if (done == child) {
            result.exited = true;
            return result;
        }
        usleep(10 * 1000);
    }
    kill(child, SIGKILL);
    waitpid(child, nullptr, 0);
    return result;
}

bool killed_by_sigterm(const ChildExit &outcome) {
    return outcome.exited && WIFSIGNALED(outcome.status) && WTERMSIG(outcome.status) == SIGTERM;
}
} // namespace

extern "C" char *msime_client_telemetry_begin(const uint8_t *, size_t) {
    const auto record = test_directory() / "session.crash";
    return copy(R"({"ok":true,"value":{"enabled":true,"crash_record_path":")" + record.string() +
                R"(","session_id":"11111111-1111-4111-8111-111111111111","crash_record_header":"","previous_session_crashed":false,"crashes":0}})");
}

extern "C" char *msime_client_telemetry_end(const uint8_t *, size_t) {
    touch(test_directory() / "ended");
    if (std::getenv(kEndHangsVariable)) sleep(60);
    return copy(R"({"ok":true,"value":true})");
}

extern "C" char *msime_client_telemetry_record_crash(const uint8_t *, size_t) {
    return copy(R"({"ok":true,"value":false})");
}

extern "C" char *msime_client_telemetry_flush(const uint8_t *, size_t) {
    return copy(R"({"ok":true,"value":{"enabled":true,"sent":0,"dropped":0,"remaining":0,"deferred":false}})");
}

extern "C" char *msime_client_telemetry_consent(const uint8_t *, size_t) {
    return copy(R"({"ok":true,"value":{"enabled":true,"epoch":"synthetic-epoch"}})");
}
extern "C" char *msime_client_telemetry_clear(const uint8_t *, size_t) {
    return copy(R"({"ok":true,"value":true})");
}
extern "C" void msime_client_string_free(char *value) { std::free(value); }

int main(int argc, char **argv) {
    if (argc > 1 && std::strcmp(argv[1], "--child") == 0) return run_child();
    char base[] = "/tmp/msime-usage-reporting-termination.XXXXXX";
    if (!mkdtemp(base)) return 1;
    const std::filesystem::path root(base);
    bool ok = true;

    // SIGTERM 结束的进程要排进 `session`，并且仍然是被 SIGTERM 结束的。
    const auto normal = root / "normal";
    ChildExit outcome = terminate_child(argv[0], normal, false, 10000);
    if (!std::filesystem::exists(normal / "ended")) ok = fail("SIGTERM did not end the usage session");
    if (!killed_by_sigterm(outcome)) ok = fail("the process did not die of SIGTERM");

    // 结束会话卡住时，进程仍要在宽限期之后被 SIGTERM 结束，不能因为接管了 SIGTERM 就杀不掉。
    const auto hanging = root / "hanging";
    outcome = terminate_child(argv[0], hanging, true, 10000);
    if (!killed_by_sigterm(outcome)) ok = fail("a stuck session end kept the process alive after SIGTERM");
    else if (outcome.elapsed_milliseconds > 6000) ok = fail("a stuck session end delayed SIGTERM beyond the grace period");

    std::filesystem::remove_all(root);
    return ok ? 0 : 1;
}
