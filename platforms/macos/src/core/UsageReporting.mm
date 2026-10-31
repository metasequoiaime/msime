#import "UsageReporting.h"
#import "EditionIdentity.h"

#import <AppKit/AppKit.h>

#include "../../../common/Telemetry.h"
#include "../../../common/CrashRecorder.h"

#include <signal.h>
#include <unistd.h>
#include <atomic>
#include <memory>
#include <mutex>

#if !__has_feature(objc_arc)
#error "UsageReporting.mm must be compiled with -fobjc-arc"
#endif

namespace
{
dispatch_queue_t gQueue;
dispatch_source_t gFlushTimer;
NSUncaughtExceptionHandler *gPreviousExceptionHandler;
std::atomic<bool> gStopped{false};
std::mutex gCaptureLifecycle;
std::unique_ptr<msime::telemetry::ConsentObserver> gConsentObserver;

// 输入法进程几乎从不走 `NSApplicationWillTerminateNotification`：设置应用装好或更新输入法时用 `pkill` 结束旧进程（`macos_input_source.rs` 的 `stop_running_copies`），系统注销、关机时结束它也可能是 SIGTERM；默认处置下进程当场死掉，会话标记留在磁盘上，下次启动按「只有标记」处理，什么事件都不产生。所以把 SIGTERM 接到这个调度源上：先排进 `session`，再按 SIGTERM 原样退出。
dispatch_source_t gTerminationSource;
// 结束会话最多等这么久。到点无论如何按 SIGTERM 退出，`pkill`、注销和关机照样能结束一个卡住的进程。
constexpr int64_t kTerminationGraceNanoseconds = 2 * NSEC_PER_SEC;

void UncaughtException(NSException *exception)
{
    // 异常原因可能引用输入内容，只保存异常类型与抛出点的栈；身份和授权来自共用采集快照。
    if (msime::crash::is_armed()) {
        NSString *summary = [NSString stringWithFormat:@"Uncaught %@", exception.name];
        NSString *stack = [exception.callStackSymbols componentsJoinedByString:@"\n"] ?: @"";
        msime::telemetry::record_exception(summary.UTF8String ?: "", stack.UTF8String ?: "");
    }
    if (gPreviousExceptionHandler) gPreviousExceptionHandler(exception);
}

void FlushNow(void)
{
    if (!gStopped.load()) msime::telemetry::flush();
}

void Flush(void) { dispatch_async(gQueue, ^{ FlushNow(); }); }

// 恢复默认处置后把 SIGTERM 发给整个进程，结束方式和退出状态与没有接管时一样，launchd 看到的仍是被 SIGTERM 结束。用 kill 而不是 raise：调度队列的工作线程可能屏蔽了 SIGTERM，发给进程才能落到没有屏蔽它的主线程上。
void ExitBySigterm(void)
{
    signal(SIGTERM, SIG_DFL);
    kill(getpid(), SIGTERM);
}

void InstallTerminationHandler(void)
{
    // 只接管本来会直接结束进程的 SIGTERM。继承来的忽略或别人装的处理函数保持原样：那种进程收到 SIGTERM 后不一定退出，提前结束会话会让之后的崩溃没有会话可归。
    struct sigaction current = {};
    if (sigaction(SIGTERM, nullptr, &current) != 0 || (current.sa_flags & SA_SIGINFO) || current.sa_handler != SIG_DFL) return;
    dispatch_queue_t queue = dispatch_queue_create("app.msime.usage-reporting.termination", DISPATCH_QUEUE_SERIAL);
    gTerminationSource = dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, SIGTERM, 0, queue);
    if (!gTerminationSource) return;
    dispatch_source_set_event_handler(gTerminationSource, ^{
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kTerminationGraceNanoseconds),
                       dispatch_get_global_queue(QOS_CLASS_USER_INTERACTIVE, 0), ^{ ExitBySigterm(); });
        MSIMEUsageReportingStop();
        ExitBySigterm();
    });
    // 调度源在 kqueue 里登记好之后才忽略 SIGTERM：先忽略的话，登记完成前到达的 SIGTERM 既不结束进程也不会被调度源看到。登记之前到达的仍按默认处置结束进程，和以前一样。
    dispatch_source_set_registration_handler(gTerminationSource, ^{ signal(SIGTERM, SIG_IGN); });
    dispatch_resume(gTerminationSource);
}
} // namespace

NSString *MSIMEUsageReportingDirectory(void)
{
#if defined(MSIME_USAGE_REPORTING_TEST_SUPPORT)
    NSURL *support = [NSURL fileURLWithPath:[NSString stringWithUTF8String:MSIME_USAGE_REPORTING_TEST_SUPPORT] isDirectory:YES];
#else
    NSURL *support = [[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask] firstObject];
#endif
    NSURL *base = support ?: [NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES];
    return [[base URLByAppendingPathComponent:MSIMEUsageReportingDirectoryName() isDirectory:YES] path];
}

static void StartReporting(NSString *preferencesDirectory, bool captureOnly)
{
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gQueue = dispatch_queue_create("app.msime.usage-reporting", DISPATCH_QUEUE_SERIAL);
        msime::telemetry::install_crash_handlers();
        if (NSGetUncaughtExceptionHandler() != UncaughtException) {
            gPreviousExceptionHandler = NSGetUncaughtExceptionHandler();
            NSSetUncaughtExceptionHandler(UncaughtException);
        }
        // 不能确定实际偏好目录时停采，不将缺失配置当作另一份默认授权。
        const std::filesystem::path preferences = [preferencesDirectory isAbsolutePath]
            ? std::filesystem::path(preferencesDirectory.UTF8String) : std::filesystem::path();
        const auto directory = MSIMEUsageReportingDirectory();
        NSString *version = [NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleShortVersionString"] ?: @"unknown";
        {
            std::lock_guard<std::mutex> guard(gCaptureLifecycle);
            if (gStopped.load()) return;
            msime::telemetry::begin({"macos", version.UTF8String, std::filesystem::path(directory.UTF8String),
                                    std::nullopt, preferences, captureOnly ? "settings-native" : "input", captureOnly});
            gConsentObserver = std::make_unique<msime::telemetry::ConsentObserver>();
        }
        InstallTerminationHandler();
        [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationWillTerminateNotification
                                                        object:nil
                                                         queue:nil
                                                    usingBlock:^(NSNotification *) { MSIMEUsageReportingStop(); }];
        Flush();
        // 发送保留三小时间隔；持续授权由独立观察线程刷新，不等待网络。
        std::lock_guard<std::mutex> guard(gCaptureLifecycle);
        if (gStopped.load()) return;
        gFlushTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, gQueue);
        const uint64_t interval = 3ull * 60 * 60 * NSEC_PER_SEC;
        dispatch_source_set_timer(gFlushTimer, dispatch_time(DISPATCH_TIME_NOW, (int64_t)interval), interval, 10ull * 60 * NSEC_PER_SEC);
        dispatch_source_set_event_handler(gFlushTimer, ^{
            FlushNow();
        });
        dispatch_resume(gFlushTimer);
    });
}

void MSIMEUsageReportingStart(NSString *preferencesDirectory)
{
    StartReporting(preferencesDirectory, false);
}

void MSIMESettingsCrashReportingStart(NSString *preferencesDirectory)
{
    StartReporting(preferencesDirectory, true);
}

void MSIMEUsageReportingStop(void)
{
    if (!gQueue) return;
    std::lock_guard<std::mutex> guard(gCaptureLifecycle);
    gStopped.store(true);
    if (gFlushTimer) dispatch_source_cancel(gFlushTimer);
    // 观察器停止并等待自己的线程，随后结束当前授权下的会话；不等待发送队列。
    gConsentObserver.reset();
    msime::telemetry::end();
}
