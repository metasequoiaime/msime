#pragma once
#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <string>

// macOS、Windows 与 Linux 原生宿主共用的上报生命周期；队列、安装身份和发送归 Host API，故障回调只落盘，重启后补报。
namespace msime::telemetry {
struct Host {
  // macos、windows 或 linux。
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
  // 准备全新状态目录时指定包内版本，避免首次读取持续同意把偏好按 full 初始化。
  // 已经写入 edition 标记的运行时宿主不需要填写它。
  std::string edition = "";
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

// 允许分配内存的语言异常回调使用当前会话快照；只保存调用方已筛选的类型和抛出点栈。
// 锁忙或没有武装时放弃，不读取偏好或联网，不随机生成新身份。
void record_exception(const std::string &summary, const std::string &stack);
// std::terminate 回调只保留异常类型和当前线程栈，不取异常消息。
void record_terminate();
// Installs crash capture that only writes to disk: SetUnhandledExceptionFilter on Windows, sigaction for SIGSEGV, SIGBUS, SIGILL, SIGFPE and SIGABRT elsewhere. POSIX handlers chain to whatever was installed before, so a host process with its own crash handler (Fcitx5) keeps it. Call once, from the main thread.
void install_crash_handlers();
// Restores the handlers install_crash_handlers replaced; for a plugin whose code may be unloaded while the process lives on.
void remove_crash_handlers();

// Exposed for tests: the crash summary of the current exception and a symbolic stack of the calling thread, as record_terminate writes them.
std::string current_exception_summary();
std::string current_stack(int skip);
}
