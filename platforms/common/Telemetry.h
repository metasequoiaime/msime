#pragma once
#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>

// Anonymous usage reporting for the native Windows and Linux hosts: a thin wrapper over the Host API's msime_client_telemetry_* functions, which own the queue, the install id, the daily active event, sessions and delivery to https://api.msime.app/v1/telemetry/events. Nothing here sends anything itself, and nothing on a crash path touches the network: crash handlers only write the session's crash record, which the next start turns into crash and session_crash events.
namespace msime::telemetry {
struct Host {
  // windows or linux.
  std::string platform;
  // The real app version.
  std::string version;
  // Where the telemetry files live (absolute); default_directory() for the IBus host and the Windows Server.
  std::filesystem::path directory;
  // 无偏好目录时的显式旧协议开关；产品有实际偏好目录时始终读取持久授权，不信缓存值。
  std::optional<bool> enabled;
  std::filesystem::path preferences_directory;
  // 组件名只含构建标识字符；独立进程只采集崩溃，不建立输入会话。
  std::string component = "input";
  bool capture_only = false;
};

// 自有辅助进程的独立采集器；本进程只能有一个，不与输入会话包装器同时使用。
// 配置读取器在启动和每 500 ms 重新执行，可返回空值表示配置不可读。析构等待观察线程结束。
// 只在已经存在的偏好目录确保持续授权，不为准备程序抢先创建其新状态目录；不联网、不创建输入会话。
// 记录与队列绑定持久授权代次，短暂撤回后重开换身份；临时读失败解除武装，同代次恢复沿用身份。
class CaptureOnly {
public:
  using ReadHost = std::function<std::optional<Host>()>;
  explicit CaptureOnly(ReadHost read_host) noexcept;
  ~CaptureOnly();
  CaptureOnly(const CaptureOnly &) = delete;
  CaptureOnly &operator=(const CaptureOnly &) = delete;

private:
  struct State;
  std::unique_ptr<State> state_;
};

// %LOCALAPPDATA%\MSIME on Windows; $XDG_STATE_HOME/msime (or ~/.local/state/msime) elsewhere.
std::filesystem::path default_directory();

// 开始本进程的上报会话；产品入口必须提供实际偏好目录，准备与记录头绑定持续授权代次。
// 关闭条件清理，不可读停采；不联网。显式 enabled 且无偏好目录仅用于旧协议兼容。
bool begin(const Host &host);
// 正常退出只结束自己的已授权会话，并停用该配置；不联网。
void end();
// 重读持久授权；不联网。读失败只停采，同代次恢复沿用身份，撤回重开换身份。
void refresh() noexcept;
// 发送前准备当前代次，每条发送由 Host API 再核对授权；只能在后台线程调用。
void flush();
// 立即发送并每三十分钟重试；网络不持本地锁。插件用自己的可等待任务，不使用此脱离线程。
void start_flushing();
// 偏好发布后的及时刷新；有实际偏好目录时不相信回调携带的旧布尔值。
void set_enabled(bool enabled);

// 每个进程一个观察器，生命周期覆盖 begin 到 end；独立于发送每 500 ms 刷新授权。
// 析构停止并等待观察线程，再结束会话；插件卸载前必须销毁，不等待网络。
class ConsentObserver {
public:
  ConsentObserver() noexcept;
  ~ConsentObserver();
  ConsentObserver(const ConsentObserver &) = delete;
  ConsentObserver &operator=(const ConsentObserver &) = delete;
private:
  struct State;
  std::unique_ptr<State> state_;
};

// For a std::terminate handler (may allocate): writes the crash record of the running session with the current exception as the message and this thread's stack. Does nothing when no session runs.
void record_terminate();
// Installs crash capture that only writes to disk: SetUnhandledExceptionFilter on Windows, sigaction for SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT elsewhere. POSIX handlers chain to whatever was installed before, so a host process with its own crash handler (Fcitx5) keeps it. Call once, from the main thread.
void install_crash_handlers();
// Restores the handlers install_crash_handlers replaced; for a plugin whose code may be unloaded while the process lives on.
void remove_crash_handlers();

// Exposed for tests: the crash summary of the current exception and a symbolic stack of the calling thread, as record_terminate writes them.
std::string current_exception_summary();
std::string current_stack(int skip);
}
