#import "UsageReporting.h"

#import <AppKit/AppKit.h>

#include "msime_client.h"

#include <execinfo.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <string.h>
#include <unistd.h>

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
    char *raw = call(static_cast<const uint8_t *>(body.bytes), body.length);
    if (!raw) return nil;
    NSData *data = [NSData dataWithBytes:raw length:strlen(raw)];
    msime_client_string_free(raw);
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

// Written once by start, read by the signal handler, which may only use async-signal-safe calls.
char gCrashRecordPath[PATH_MAX];
const int kCrashSignals[] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT, SIGTRAP};
struct sigaction gPreviousActions[NSIG];
// Room to report a stack overflow on the main thread.
char gAlternateStack[64 * 1024];

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

void Flush(void)
{
    dispatch_async(gQueue, ^{
        (void)CallHost(msime_client_telemetry_flush, SessionRequest());
    });
}

const char *SignalName(int signal)
{
    switch (signal) {
    case SIGSEGV: return "SIGSEGV";
    case SIGBUS: return "SIGBUS";
    case SIGILL: return "SIGILL";
    case SIGFPE: return "SIGFPE";
    case SIGABRT: return "SIGABRT";
    case SIGTRAP: return "SIGTRAP";
    default: return "signal";
    }
}

// Only async-signal-safe work: open the record begin returned, write the summary line and the frames (backtrace_symbols_fd writes "index module address symbol + offset" without allocating, and names a module by its file name only), then hand the signal to whoever had it before.
void CrashSignalHandler(int signal, siginfo_t *, void *)
{
    if (gCrashRecordPath[0] != '\0') {
        int fd = open(gCrashRecordPath, O_WRONLY | O_CREAT | O_EXCL, 0600);
        if (fd >= 0) {
            const char *prefix = "Fatal signal ";
            const char *name = SignalName(signal);
            (void)!write(fd, prefix, strlen(prefix));
            (void)!write(fd, name, strlen(name));
            (void)!write(fd, "\n", 1);
            void *frames[64];
            int count = backtrace(frames, 64);
            backtrace_symbols_fd(frames, count, fd);
            close(fd);
        }
    }
    sigaction(signal, &gPreviousActions[signal], nullptr);
    raise(signal);
}

void UncaughtException(NSException *exception)
{
    NSString *summary = [NSString stringWithFormat:@"Uncaught %@: %@", exception.name, exception.reason ?: @""];
    NSString *stack = [exception.callStackSymbols componentsJoinedByString:@"\n"] ?: @"";
    (void)CallHost(msime_client_telemetry_record_crash,
                   @{@"directory": MSIMEUsageReportingDirectory(), @"message": summary, @"stack": stack});
    if (gPreviousExceptionHandler) gPreviousExceptionHandler(exception);
}

void InstallCrashHandlers(NSString *recordPath)
{
    if (![recordPath getFileSystemRepresentation:gCrashRecordPath maxLength:sizeof(gCrashRecordPath)]) {
        gCrashRecordPath[0] = '\0';
        return;
    }
    // The first backtrace() loads the unwinder, which allocates; do it here rather than inside a signal handler.
    void *warm[1];
    (void)backtrace(warm, 1);
    stack_t alternate = {};
    alternate.ss_sp = gAlternateStack;
    alternate.ss_size = sizeof(gAlternateStack);
    sigaltstack(&alternate, nullptr);
    struct sigaction action = {};
    action.sa_sigaction = CrashSignalHandler;
    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    for (int signal : kCrashSignals) sigaction(signal, &action, &gPreviousActions[signal]);
    gPreviousExceptionHandler = NSGetUncaughtExceptionHandler();
    NSSetUncaughtExceptionHandler(UncaughtException);
}
} // namespace

NSString *MSIMEUsageReportingDirectory(void)
{
    NSURL *support = [[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask] firstObject];
    NSURL *base = support ?: [NSURL fileURLWithPath:NSTemporaryDirectory() isDirectory:YES];
    return [[base URLByAppendingPathComponent:@"MSIME/telemetry" isDirectory:YES] path];
}

void MSIMEUsageReportingStart(NSString *preferencesDirectory)
{
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        gPreferencesDirectory = [preferencesDirectory copy];
        gQueue = dispatch_queue_create("app.msime.usage-reporting", DISPATCH_QUEUE_SERIAL);
        [NSFileManager.defaultManager createDirectoryAtPath:MSIMEUsageReportingDirectory()
                                withIntermediateDirectories:YES
                                                 attributes:@{NSFilePosixPermissions: @0700}
                                                      error:nil];
        NSDictionary *started = CallHost(msime_client_telemetry_begin, SessionRequest());
        if (![started isKindOfClass:NSDictionary.class] || ![started[@"enabled"] isEqual:@YES]) return;
        NSString *recordPath = started[@"crash_record_path"];
        if ([recordPath isKindOfClass:NSString.class]) InstallCrashHandlers(recordPath);
        [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationWillTerminateNotification
                                                        object:nil
                                                         queue:nil
                                                    usingBlock:^(NSNotification *) { MSIMEUsageReportingStop(); }];
        Flush();
        // The input method runs for days: flush every few hours so a new UTC day's active and anything a failed send left behind go out. With nothing queued and today's active already sent, a flush does no network I/O.
        gFlushTimer = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER, 0, 0, gQueue);
        const uint64_t interval = 3ull * 60 * 60 * NSEC_PER_SEC;
        dispatch_source_set_timer(gFlushTimer, dispatch_time(DISPATCH_TIME_NOW, (int64_t)interval), interval, 10ull * 60 * NSEC_PER_SEC);
        dispatch_source_set_event_handler(gFlushTimer, ^{
            (void)CallHost(msime_client_telemetry_flush, SessionRequest());
        });
        dispatch_resume(gFlushTimer);
    });
}

void MSIMEUsageReportingStop(void)
{
    if (!gQueue) return;
    (void)CallHost(msime_client_telemetry_end, @{@"directory": MSIMEUsageReportingDirectory()});
}
