#import "UsageReporting.h"
#import "EditionIdentity.h"

#import <AppKit/AppKit.h>

#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../../../common/CrashRecorder.h"

#include <execinfo.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <string.h>
#include <unistd.h>
#include <atomic>
#include <memory>
#include <mutex>

#if !__has_feature(objc_arc)
#error "UsageReporting.mm must be compiled with -fobjc-arc"
#endif

namespace
{
using HostCall = char *(*)(const uint8_t *, size_t);

// The value of the {ok,value} envelope, or nil on any failure. Usage reporting never affects typing, so a failure is only logged.
id CallHost(HostCall call, NSDictionary *request)
{
    NSData *body = [NSJSONSerialization dataWithJSONObject:request options:0 error:nil];
    if (!body) return nil;
    auto raw = msime::host_api::own_string(
        call(static_cast<const uint8_t *>(body.bytes), body.length));
    if (!raw) return nil;
    NSData *data = [NSData dataWithBytes:raw.get() length:strlen(raw.get())];
    NSDictionary *envelope = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if (![envelope isKindOfClass:NSDictionary.class]) return nil;
    if ([envelope[@"ok"] isEqual:@YES]) return envelope[@"value"];
    NSLog(@"MSIME usage reporting: %@", envelope[@"error"]);
    return nil;
}

NSString *gPreferencesDirectory;
dispatch_queue_t gQueue;
dispatch_source_t gFlushTimer;
NSUncaughtExceptionHandler *gPreviousExceptionHandler;

struct Capture {
    NSString *session;
    NSString *header;
};
// 刷新队列可换入新的会话；退出和异常回调读取不可变快照，不并发改写字符串。
std::shared_ptr<const Capture> gCapture;
std::atomic<bool> gStopped{false};
std::mutex gCaptureLifecycle;

// 输入法进程几乎从不走 `NSApplicationWillTerminateNotification`：设置应用装好或更新输入法时用 `pkill` 结束旧进程（`macos_input_source.rs` 的 `stop_running_copies`），系统注销、关机时结束它也可能是 SIGTERM；默认处置下进程当场死掉，会话标记留在磁盘上，下次启动按「只有标记」处理，什么事件都不产生。所以把 SIGTERM 接到这个调度源上：先排进 `session`，再按 SIGTERM 原样退出。
dispatch_source_t gTerminationSource;
// 结束会话最多等这么久。到点无论如何按 SIGTERM 退出，`pkill`、注销和关机照样能结束一个卡住的进程。
constexpr int64_t kTerminationGraceNanoseconds = 2 * NSEC_PER_SEC;

NSDictionary *SessionRequest(void)
{
    NSMutableDictionary *request = [@{
        @"directory": MSIMEUsageReportingDirectory(),
        @"platform": @"macos",
        @"version": [NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleShortVersionString"] ?: @"unknown",
    } mutableCopy];
    // The shared settings page writes usage_reporting there; before the host is configured the default (on) applies.
    if (gPreferencesDirectory.length > 0) request[@"preferences_directory"] = gPreferencesDirectory;
    else request[@"enabled"] = @YES;
    return request;
}

void UncaughtException(NSException *exception)
{
    const auto capture = std::atomic_load(&gCapture);
    if (msime::crash::is_armed() && capture) {
        // 异常原因可能引用输入内容，只保存异常类型与抛出点的栈。
        NSString *summary = [NSString stringWithFormat:@"Uncaught %@", exception.name];
        NSString *stack = [exception.callStackSymbols componentsJoinedByString:@"\n"] ?: @"";
        (void)CallHost(msime_client_telemetry_record_crash,
                       @{@"directory": MSIMEUsageReportingDirectory(), @"message": summary, @"stack": stack,
                         @"session": capture->session, @"crash_record_header": capture->header});
    }
    if (gPreviousExceptionHandler) gPreviousExceptionHandler(exception);
}

void InstallCrashHandlers(NSString *recordPath, NSString *header)
{
    msime::crash::install_crash_handlers();
    msime::crash::arm(recordPath.UTF8String ?: "", header.UTF8String ?: "");
    // 再次开启只恢复采集，不能把自己接成「前一个」处理程序而递归调用。
    if (NSGetUncaughtExceptionHandler() != UncaughtException) {
        gPreviousExceptionHandler = NSGetUncaughtExceptionHandler();
        NSSetUncaughtExceptionHandler(UncaughtException);
    }
}

void PrepareCapture(NSDictionary *started)
{
    if (![started isKindOfClass:NSDictionary.class] || ![started[@"enabled"] isEqual:@YES]) return;
    NSString *session = started[@"session_id"];
    NSString *recordPath = started[@"crash_record_path"];
    NSString *header = started[@"crash_record_header"];
    if (![session isKindOfClass:NSString.class] || ![recordPath isKindOfClass:NSString.class] || ![header isKindOfClass:NSString.class]) return;
    {
        // begin 可以在退出前发起、退出后才回答；发布与 Stop 在同一把普通路径锁内线性化。
        std::lock_guard<std::mutex> guard(gCaptureLifecycle);
        if (!gStopped.load()) {
            std::shared_ptr<const Capture> capture = std::make_shared<Capture>(Capture{[session copy], [header copy]});
            std::atomic_store(&gCapture, std::move(capture));
            InstallCrashHandlers(recordPath, header);
            return;
        }
    }
    // 退出后才拿到的会话只结束自己的标记，Host 调用不跨生命周期锁。
    (void)CallHost(msime_client_telemetry_end, @{@"directory": MSIMEUsageReportingDirectory(), @"session": session});
}

void FlushNow(void)
{
    if (gStopped.load()) return;
    NSDictionary *result = CallHost(msime_client_telemetry_flush, SessionRequest());
    if (![result isKindOfClass:NSDictionary.class]) return;
    if ([result[@"enabled"] isEqual:@NO]) {
        msime::crash::disarm();
        std::atomic_store(&gCapture, std::shared_ptr<const Capture>{});
    } else if ([result[@"enabled"] isEqual:@YES] && !msime::crash::is_armed()) {
        // 关闭时共享层已清掉旧会话和记录位置，重新开启要取得新的采集位置。
        PrepareCapture(CallHost(msime_client_telemetry_begin, SessionRequest()));
    }
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
    NSURL *support = [NSURL fileURLWithPath:@MSIME_USAGE_REPORTING_TEST_SUPPORT isDirectory:YES];
#else
    NSURL *support = [[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask] firstObject];
#endif
    NSURL *base = support ?: [NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES];
    return [[base URLByAppendingPathComponent:MSIMEUsageReportingDirectoryName() isDirectory:YES] path];
}

void MSIMEUsageReportingStart(NSString *preferencesDirectory)
{
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gPreferencesDirectory = [preferencesDirectory copy];
        gQueue = dispatch_queue_create("app.msime.usage-reporting", DISPATCH_QUEUE_SERIAL);
        // host-api telemetry 存储会在接触队列、标记或崩溃文件前创建并校验目录。
        NSDictionary *started = CallHost(msime_client_telemetry_begin, SessionRequest());
        PrepareCapture(started);
        InstallTerminationHandler();
        [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationWillTerminateNotification
                                                        object:nil
                                                         queue:nil
                                                    usingBlock:^(NSNotification *) { MSIMEUsageReportingStop(); }];
        Flush();
        // 输入法可长驻数天，关闭时也保留刷新定时器，以便再次开启后恢复会话与采集。
        gFlushTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, gQueue);
        const uint64_t interval = 3ull * 60 * 60 * NSEC_PER_SEC;
        dispatch_source_set_timer(gFlushTimer, dispatch_time(DISPATCH_TIME_NOW, (int64_t)interval), interval, 10ull * 60 * NSEC_PER_SEC);
        dispatch_source_set_event_handler(gFlushTimer, ^{
            FlushNow();
        });
        dispatch_resume(gFlushTimer);
    });
}

void MSIMEUsageReportingStop(void)
{
    if (!gQueue) return;
    std::shared_ptr<const Capture> capture;
    {
        std::lock_guard<std::mutex> guard(gCaptureLifecycle);
        gStopped.store(true);
        msime::crash::disarm();
        capture = std::atomic_exchange(&gCapture, std::shared_ptr<const Capture>{});
    }
    if (capture) (void)CallHost(msime_client_telemetry_end, @{@"directory": MSIMEUsageReportingDirectory(), @"session": capture->session});
}
