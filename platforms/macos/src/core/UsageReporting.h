#import <Foundation/Foundation.h>

// 输入法进程的匿名使用情况报告，经 client-services 的遥测队列（`msime_client.h` 里的 `msime_client_telemetry_*`）。输入法进程的一次生命周期就是一个会话；共享偏好里的 `usage_reporting`（默认开启）决定是否记录或发送任何内容。

// 开始会话，装上崩溃处理程序和 SIGTERM 处理，再从后台队列发送队列里的事件。在运行时配置就绪之后、接受 IMK 会话之前调用一次。preferencesDirectory 是运行时配置里的共享偏好目录，宿主还没配置时传 nil（这时按默认值，即开启）。进程被 SIGTERM 结束（设置应用安装或更新时的 `pkill`，或系统注销、关机时发来的 SIGTERM）和正常退出一样算会话正常结束：先排进 `session`，再按 SIGTERM 退出。
void MSIMEUsageReportingStart(NSString *_Nullable preferencesDirectory);

// 进程正常结束：排进本次会话的 `session` 事件。可以调用多次。
void MSIMEUsageReportingStop(void);

// The directory holding the queue, the install id, the session marker and crash records.
NSString *_Nonnull MSIMEUsageReportingDirectory(void);
