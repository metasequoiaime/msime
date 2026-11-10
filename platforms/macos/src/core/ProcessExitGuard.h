#pragma once

// 输入法进程不论怎么结束，都以 SIGKILL 收尾。
//
// macOS 的 imklaunchagent 给每个输入法记死亡次数：30 分钟内满 11 次就广播 `com.apple.inputmethodkit.abnormaldeath`，此后所有正在运行的应用都拒绝这个输入法，它的模式在输入菜单里变灰，修复、重装、升级都解除不了，只能注销重登或重启电脑。崩溃、被 SIGTERM 结束、`exit(0)` 正常退出都计数，只有退出状态是 9（被 SIGKILL 结束）的那次不计（macOS 27.2 反汇编与实测：SIGKILL 结束后 imklaunchagent 不记 `IMDiedCallback`，SIGTERM 结束记 `exitStatus (15)`）。连续崩溃的版本、反复点「修复输入源」的用户都能攒满 11 次，所以输入法进程的每条退出路径都改成最后给自己发 SIGKILL。
//
// 代价是系统不再为输入法的崩溃生成 `~/Library/Logs/DiagnosticReports` 下的报告，崩溃栈只剩本进程的崩溃记录器写下的那份。

#ifdef __cplusplus
extern "C" {
#endif

// 只在输入法进程里调用一次，要早于崩溃记录器安装（`MSIMEUsageReportingStart`）：记录器把这里的处理函数当作前任，写完记录后才交给它。独立设置窗口和命令行辅助模式不是 imklaunchagent 拉起的输入法，不调用。
void MSIMEInstallProcessExitGuard(void);

// 立刻以 SIGKILL 结束本进程。可以在信号处理函数里调用。
void MSIMEExitByKill(void) __attribute__((noreturn));

#ifdef __cplusplus
}
#endif
