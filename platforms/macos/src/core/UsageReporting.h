#import <Foundation/Foundation.h>

// 输入法进程的匿名使用情况报告，经 client-services 的遥测队列（`msime_client.h` 里的 `msime_client_telemetry_*`）。输入法进程的一次生命周期就是一个会话；共享偏好里的 `usage_reporting`（默认开启）决定是否记录或发送任何内容。

// Starts the session, installs the crash handlers and sends what is queued from a background queue. Call once, after the runtime options are current and before IMK sessions are accepted. preferencesDirectory is the shared preferences directory from the runtime options, or nil when the host is not configured yet (reporting then follows the default, on).
void MSIMEUsageReportingStart(NSString *_Nullable preferencesDirectory);

// The process is exiting normally: queues the session event. Safe to call more than once.
void MSIMEUsageReportingStop(void);

// The directory holding the queue, the install id, the session marker and crash records.
NSString *_Nonnull MSIMEUsageReportingDirectory(void);
