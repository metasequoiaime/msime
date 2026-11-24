#pragma once
#include "PipeTicket.h"
#include <windows.h>
#include <condition_variable>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <utility>

namespace msime::windows {
namespace desktop_identity {
// Win7 没有对象比较接口；专用无窗口线程用实际绑定后的桌面根窗口确认身份。
// 进程内共用探针并串行请求，避免在线程本地对象析构中等待另一线程的退出。
class BindingProbe final {
public:
  BindingProbe() : worker_([this] { run(); }) {}
  ~BindingProbe() {
    { std::lock_guard lock(mutex_); stopping_ = true; }
    changed_.notify_one();
    worker_.join();
  }
  bool same(HDESK first, HDESK second) {
    std::lock_guard request(request_mutex_);
    std::unique_lock lock(mutex_);
    first_ = first; second_ = second; pending_ = true; complete_ = false;
    changed_.notify_one();
    completed_.wait(lock, [this] { return complete_; });
    return result_;
  }
private:
  void run() {
    std::unique_lock lock(mutex_);
    for (;;) {
      changed_.wait(lock, [this] { return pending_ || stopping_; });
      if (stopping_) return;
      const auto first = first_, second = second_;
      pending_ = false;
      lock.unlock();
      bool result = false;
      const auto original = GetThreadDesktop(GetCurrentThreadId());
      if (original && SetThreadDesktop(first)) {
        const auto root = GetDesktopWindow();
        if (root && SetThreadDesktop(second)) result = root == GetDesktopWindow();
        // 返回前恢复借用桌面，调用方随后可以关闭自己的拥有句柄。
        if (!SetThreadDesktop(original)) result = false;
      }
      lock.lock();
      result_ = result; complete_ = true;
      completed_.notify_one();
    }
  }
  std::mutex request_mutex_, mutex_;
  std::condition_variable changed_, completed_;
  HDESK first_ = nullptr, second_ = nullptr;
  bool pending_ = false, complete_ = false, result_ = false, stopping_ = false;
  std::thread worker_;
};
inline bool same_by_binding(HDESK first, HDESK second) {
  if (!first || !second) return false;
  try {
    static BindingProbe probe;
    return probe.same(first, second);
  } catch (...) { return false; }
}
} // namespace desktop_identity

// 桌面句柄不能按数值或名字比较：不同窗口站可以有同名桌面。
inline bool same_desktop_object(HDESK first, HDESK second) {
  using Compare = BOOL(WINAPI *)(HANDLE, HANDLE);
  static const auto compare = reinterpret_cast<Compare>(reinterpret_cast<void *>(
      GetProcAddress(GetModuleHandleW(L"kernelbase.dll"), "CompareObjectHandles")));
  if (!first || !second) return false;
  if (compare) return compare(first, second) != FALSE;
  // 实际绑定同时核对进程窗口站，不能把另一窗口站的同名桌面误当成目标。
  return desktop_identity::same_by_binding(first, second);
}

// 只接受来自当前焦点租约的已认证 PipeTicket：PipePeer 已核对高 32 位与真实管道对端。
// 这里再核对低 32 位的线程归属，持有线程对象，并拥有供 UI 线程绑定的桌面句柄。
class ClientDesktop final {
public:
  // 客户端桌面解析不到时的回落：Server 自己所在的桌面。低完整性宿主（Windows 搜索框等）上
  // 拿不到客户端线程的桌面归属，候选窗不能因此整个不出现（#11793）；跨桌面场景下候选窗
  // 最多出现在别的桌面上，而不是没有候选窗。
  static std::optional<ClientDesktop> server_desktop() {
    ClientDesktop result;
    result.desktop_ = open_named(GetThreadDesktop(GetCurrentThreadId()));
    if (!result.desktop_)
      return std::nullopt;
    result.fallback_ = true;
    return result;
  }
  // 优先按票据解析客户端自己的桌面。只有系统拒绝 Server 读取或打开客户端桌面（ERROR_ACCESS_DENIED）、且客户端线程仍在运行时才退到 Server 桌面；票据认证不通过、线程已退出、桌面身份对不上（另一窗口站的同名桌面）一律返回空，不回落。
  static std::optional<ClientDesktop> resolve_or_server(const PipeTicket &ticket, std::string *why = nullptr) {
    auto authenticated = authenticate(ticket, why);
    if (!authenticated)
      return std::nullopt;
    bool denied = false;
    if (authenticated->attach_desktop(why, &denied))
      return authenticated;
    if (!denied || !authenticated->alive())
      return std::nullopt;
    auto server = server_desktop();
    if (!server) {
      if (why) *why = "server desktop unavailable";
      return std::nullopt;
    }
    // 回落后仍持有客户端线程：线程退出时 current() 失效，候选窗不会一直留在 Server 桌面上。
    server->thread_ = std::exchange(authenticated->thread_, nullptr);
    server->tid_ = authenticated->tid_;
    if (!why) return server;
    if (!why->empty()) *why += "; ";
    *why += "using the server desktop";
    return server;
  }
  static std::optional<ClientDesktop> resolve(const PipeTicket &ticket, std::string *why = nullptr) {
    auto authenticated = authenticate(ticket, why);
    if (!authenticated)
      return std::nullopt;
    if (!authenticated->attach_desktop(why))
      return std::nullopt;
    return authenticated;
  }
  ClientDesktop(ClientDesktop &&other) noexcept
      : thread_(std::exchange(other.thread_, nullptr)),
        desktop_(std::exchange(other.desktop_, nullptr)), tid_(other.tid_),
        fallback_(other.fallback_) {}
  ClientDesktop &operator=(ClientDesktop &&other) noexcept {
    if (this != &other) {
      close();
      thread_ = std::exchange(other.thread_, nullptr);
      desktop_ = std::exchange(other.desktop_, nullptr);
      tid_ = other.tid_;
      fallback_ = other.fallback_;
    }
    return *this;
  }
  ClientDesktop(const ClientDesktop &) = delete;
  ClientDesktop &operator=(const ClientDesktop &) = delete;
  ~ClientDesktop() { close(); }
  HDESK handle() const { return desktop_; }
  bool fallback() const { return fallback_; }
  // 线程退出或换了桌面时，缓存的归属立即失效；回落到 Server 桌面时句柄就是 Server 自己那块桌面，只要客户端线程（若有）仍在运行就一直有效。
  bool current() const {
    if (fallback_)
      return desktop_ != nullptr && (!thread_ || alive());
    return alive() && same_desktop_object(GetThreadDesktop(tid_), desktop_);
  }

private:
  static constexpr DWORD kDesktopRights = DESKTOP_CREATEWINDOW | DESKTOP_CREATEMENU |
      DESKTOP_ENUMERATE | DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS;
  // 只核对票据：高 32 位的进程已由 PipePeer 对过管道对端，这里核对低 32 位线程属于该进程且仍在运行。
  // 这一步失败说明票据不可信，任何回落都不允许。
  static std::optional<ClientDesktop> authenticate(const PipeTicket &ticket, std::string *why = nullptr) {
    const auto fail = [why](const char *stage) {
      if (why) *why = stage;
      return std::optional<ClientDesktop>{};
    };
    const DWORD pid = client_pid(ticket);
    const DWORD tid = static_cast<DWORD>(ticket.client);
    if (!pid || !tid)
      return fail("ticket without client thread");
    ClientDesktop result;
    result.thread_ = OpenThread(THREAD_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, tid);
    if (!result.thread_ || GetProcessIdOfThread(result.thread_) != pid || !result.alive())
      return fail("client thread unavailable");
    result.tid_ = tid;
    return result;
  }
  // 在已认证的线程上解析桌面归属，并打开一个拥有句柄。
  // denied 只在系统拒绝读取或打开客户端桌面时置真，供 resolve_or_server 判断能否回落。
  bool attach_desktop(std::string *why = nullptr, bool *denied = nullptr) {
    // GetThreadDesktop 返回借用句柄，不能 CloseDesktop；也不能直接据它跨窗口站绑定线程。
    std::string error;
    const HDESK assigned = GetThreadDesktop(tid_);
    desktop_ = open_named(assigned, &error, denied);
    if (!desktop_) {
      if (why) *why = std::move(error);
      return false;
    }
    if (!alive()) {
      close();
      if (why) *why = "client thread exited";
      return false;
    }
    if (!same_desktop_object(assigned, desktop_)) {
      close();
      if (why) *why = "client desktop identity mismatch";
      return false;
    }
    return true;
  }
  // 按名字在当前窗口站打开一个拥有句柄；失败时把阶段和系统错误写给诊断。
  static HDESK open_named(HDESK assigned, std::string *error = nullptr, bool *denied = nullptr) {
    wchar_t name[256]{};
    DWORD needed = 0;
    if (!assigned) {
      // 调用方紧接在 GetThreadDesktop 之后调用这里，此时的 GetLastError 还是它留下的。
      const DWORD code = GetLastError();
      if (denied) *denied = code == ERROR_ACCESS_DENIED;
      if (error) *error = "desktop query failed, error=" + std::to_string(code);
      return nullptr;
    }
    if (!GetUserObjectInformationW(assigned, UOI_NAME, name, sizeof(name), &needed)) {
      const DWORD code = GetLastError();
      if (denied) *denied = code == ERROR_ACCESS_DENIED;
      if (error) *error = "desktop name query failed, error=" + std::to_string(code);
      return nullptr;
    }
    if (!needed || needed > sizeof(name) || !name[0]) {
      if (error) *error = "desktop name empty";
      return nullptr;
    }
    const HDESK opened = OpenDesktopW(name, 0, FALSE, kDesktopRights);
    if (!opened) {
      const DWORD code = GetLastError();
      if (denied) *denied = code == ERROR_ACCESS_DENIED;
      if (error) *error = "desktop open failed, error=" + std::to_string(code);
    }
    return opened;
  }
  ClientDesktop() = default;
  bool alive() const { return thread_ && WaitForSingleObject(thread_, 0) == WAIT_TIMEOUT; }
  void close() {
    if (desktop_)
      CloseDesktop(std::exchange(desktop_, nullptr));
    if (thread_)
      CloseHandle(std::exchange(thread_, nullptr));
  }
  HANDLE thread_ = nullptr;
  HDESK desktop_ = nullptr;
  DWORD tid_ = 0;
  bool fallback_ = false;
};
} // namespace msime::windows
