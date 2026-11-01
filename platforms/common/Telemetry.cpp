#include "Telemetry.h"
#include "CrashRecorder.h"
#include "HostApiString.h"
#include "msime_client.h"
#include <nlohmann/json.hpp>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdlib>
#include <cstring>
#include <exception>
#include <iterator>
#include <memory>
#include <mutex>
#include <sstream>
#include <thread>
#include <typeinfo>
#ifdef _WIN32
#include <vector>
#include <windows.h>
#else
#include <cerrno>
#include <csignal>
#include <fcntl.h>
#include <unistd.h>
#if __has_include(<execinfo.h>)
#include <execinfo.h>
#define MSIME_TELEMETRY_BACKTRACE 1
#endif
#if __has_include(<cxxabi.h>)
#include <cxxabi.h>
#define MSIME_TELEMETRY_DEMANGLE 1
#endif
#endif

#if defined(MSIME_TELEMETRY_TEST_FLUSH)
// 仅故障测试替代网络入口；准备、授权与队列仍走真实 Host API。
extern "C" char *msime_test_telemetry_flush(const uint8_t *, size_t);
#endif
#if defined(MSIME_TELEMETRY_TEST_BEGIN)
// 仅故障测试暂停真实 begin 返回后的发布，不替代持久授权与队列事务。
extern "C" char *msime_test_telemetry_begin(const uint8_t *, size_t);
#endif

#if defined(MSIME_TELEMETRY_TEST_CAPTURE)
// 只在独立采集回归中暂停发布，实际授权读取与记录器保持生产实现。
extern "C" void msime_test_capture_before_arm();
extern "C" void msime_test_capture_after_refresh();
#endif

namespace msime::telemetry {
namespace {
using Request = char *(*)(const uint8_t *, size_t);

std::mutex host_lock;
Host current;
bool configured = false;
// 本进程正在运行的会话 id（begin 返回的 session_id），由 host_lock 保护；没有会话时为空。结束会话和 terminate 路径的崩溃记录都按它找自己的会话：同一目录可能还有别的宿主进程（例如同一用户的另一个图形会话），标记文件只记着最后开始的那个。
std::string session_id;
std::string capture_header;
std::string capture_path;
std::string consent_epoch;
bool disabled_cleared = false;
std::atomic<std::terminate_handler> auxiliary_previous_terminate{nullptr};

[[noreturn]] void auxiliary_terminate() noexcept {
  try {
    // Windows 的 CRT abort 不保证经过 SEH 过滤器，必须在交还原处理器前记录固定异常类型。
    const auto summary = current_exception_summary();
    crash::record(summary.c_str());
  } catch (...) {
  }
  const auto previous = auxiliary_previous_terminate.load();
  if (previous && previous != auxiliary_terminate)
    previous();
  std::abort();
}

// Calls one Host API function with a JSON request; the value on success, nothing on any failure. Reporting never takes the host down.
std::optional<nlohmann::json> call(Request function, const nlohmann::json &request) {
  try {
    const auto body = request.dump();
    auto response = msime::host_api::own_string(
        function(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
    if (!response)
      return std::nullopt;
    auto parsed = nlohmann::json::parse(response.get(), nullptr, false);
    if (parsed.is_discarded() || !parsed.value("ok", false) || !parsed.contains("value"))
      return std::nullopt;
    return std::move(parsed["value"]);
  } catch (...) {
    return std::nullopt;
  }
}

std::string utf8(const std::filesystem::path &path) {
  const auto value = path.u8string();
  return std::string(value.begin(), value.end());
}

nlohmann::json session_request(const Host &host) {
  nlohmann::json request{{"directory", utf8(host.directory)}, {"platform", host.platform}, {"version", host.version}};
  request["component"] = host.component;
  request["capture_only"] = host.capture_only;
  if (host.enabled)
    request["enabled"] = *host.enabled;
  else if (!host.preferences_directory.empty())
    request["preferences_directory"] = utf8(host.preferences_directory);
  else
    request["enabled"] = true;
  return request;
}

nlohmann::json consent_request(const Host &host) {
  nlohmann::json request{{"directory", utf8(host.preferences_directory)}};
  if (!host.edition.empty())
    request["edition"] = host.edition;
  return request;
}

void forget_capture_locked() {
  crash::disarm();
  session_id.clear();
  capture_header.clear();
  capture_path.clear();
  consent_epoch.clear();
}

std::optional<nlohmann::json> consent_locked() {
  if (!current.preferences_directory.is_absolute())
    return std::nullopt;
  const auto value = call(msime_client_telemetry_consent, consent_request(current));
  if (!value || !value->is_object() || !value->contains("enabled") || !(*value)["enabled"].is_boolean())
    return std::nullopt;
  return value;
}

nlohmann::json bound_session_request() {
  auto request = session_request(current);
  if (!consent_epoch.empty()) {
    request.erase("enabled");
    request["preferences_directory"] = utf8(current.preferences_directory);
    request["consent_epoch"] = consent_epoch;
  }
  return request;
}

bool consent_matches_locked() {
  if (consent_epoch.empty())
    return true;
  const auto consent = consent_locked();
  return consent && consent->value("enabled", false) &&
         consent->value("epoch", std::string()) == consent_epoch;
}

bool refresh_locked() {
  if (!configured || !current.directory.is_absolute())
    return false;
  std::string epoch;
  if (!current.preferences_directory.empty()) {
    const auto consent = consent_locked();
    if (!consent) {
      // 临时不可读仅停采；同一持续授权恢复后重新武装原快照。
      crash::disarm();
      return false;
    }
    if (!(*consent)["enabled"].get<bool>()) {
      forget_capture_locked();
      if (!disabled_cleared) {
        const auto cleared = call(msime_client_telemetry_clear,
            nlohmann::json{{"directory", utf8(current.directory)},
                           {"preferences_directory", utf8(current.preferences_directory)}});
        disabled_cleared = cleared && cleared->is_boolean() && cleared->get<bool>();
      }
      return false;
    }
    const auto found = consent->find("epoch");
    if (found == consent->end() || !found->is_string() || found->get_ref<const std::string &>().empty()) {
      crash::disarm();
      return false;
    }
    epoch = found->get<std::string>();
  } else if (!current.enabled.value_or(true)) {
    // 没有偏好目录且未携带开关时沿用旧协议的默认开启；只有显式 false 才停采并清理。
    forget_capture_locked();
    if (current.enabled == false && !disabled_cleared) {
      const auto cleared = call(msime_client_telemetry_clear,
          nlohmann::json{{"directory", utf8(current.directory)}});
      disabled_cleared = cleared && cleared->is_boolean() && cleared->get<bool>();
    }
    return false;
  }
  disabled_cleared = false;
  if (!session_id.empty() && consent_epoch == epoch && !capture_path.empty()) {
    if (!crash::is_armed()) {
      if (!consent_matches_locked())
        return false;
      crash::arm(capture_path, capture_header);
    }
    return crash::is_armed();
  }
  forget_capture_locked();
  consent_epoch = epoch;
#if defined(MSIME_TELEMETRY_TEST_BEGIN)
  const auto value = call(msime_test_telemetry_begin, bound_session_request());
#else
  const auto value = call(msime_client_telemetry_begin, bound_session_request());
#endif
  if (!value || !value->is_object() || !value->value("enabled", false)) {
    forget_capture_locked();
    return false;
  }
  const auto id = value->find("session_id");
  const auto path = value->find("crash_record_path");
  const auto header = value->find("crash_record_header");
  if (id == value->end() || !id->is_string() || path == value->end() || !path->is_string() ||
      header == value->end() || !header->is_string()) {
    forget_capture_locked();
    return false;
  }
  session_id = id->get<std::string>();
  capture_path = path->get<std::string>();
  capture_header = header->get<std::string>();
  // begin 已释放共享偏好锁；慢回包不能重新武装已经撤回或换代的头。
  if (!consent_matches_locked())
    return false;
  crash::arm(capture_path, capture_header);
  return crash::is_armed();
}

#ifndef _WIN32
#ifdef MSIME_TELEMETRY_DEMANGLE
std::string demangle(const char *name) {
  int status = 0;
  std::unique_ptr<char, decltype(&std::free)> readable(abi::__cxa_demangle(name, nullptr, nullptr, &status), std::free);
  return status == 0 && readable ? std::string(readable.get()) : std::string(name);
}
#else
std::string demangle(const char *name) { return name; }
#endif
#else
std::string demangle(const char *name) { return name; }
#endif
} // namespace

std::filesystem::path default_directory() {
#ifdef _WIN32
  // Read the wide value through Win32: getenv is deprecated under MSVC /WX and would pass the path through the ANSI code page, and MinGW's msvcrt import library has no _wdupenv_s.
  std::vector<wchar_t> base(32768);
  const DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA", base.data(), static_cast<DWORD>(base.size()));
  if (!length || length >= base.size())
    return {};
  return std::filesystem::path(std::wstring(base.data(), length)) / "MSIME";
#else
  if (const char *state = std::getenv("XDG_STATE_HOME"); state && *state == '/')
    return std::filesystem::path(state) / "msime";
  if (const char *home = std::getenv("HOME"); home && *home == '/')
    return std::filesystem::path(home) / ".local/state/msime";
  return {};
#endif
}

bool begin(const Host &host) {
  std::lock_guard guard(host_lock);
  forget_capture_locked();
  current = host;
  configured = true;
  disabled_cleared = false;
  return refresh_locked();
}

void end() {
  std::lock_guard guard(host_lock);
  if (!configured)
    return;
  configured = false;
  crash::disarm();
  bool allowed = true;
  if (!current.preferences_directory.empty()) {
    const auto consent = consent_locked();
    allowed = consent && consent->value("enabled", false) &&
              consent->value("epoch", std::string()) == consent_epoch;
    if (consent && !consent->value("enabled", true))
      call(msime_client_telemetry_clear,
          nlohmann::json{{"directory", utf8(current.directory)},
                         {"preferences_directory", utf8(current.preferences_directory)}});
  }
  // 只结束已授权的自身会话，不激活代次。新队列清理后已无旧会话 id，迟到的 end 不会写入新队列。
  if (allowed && !session_id.empty() && !current.capture_only)
    call(msime_client_telemetry_end, nlohmann::json{{"directory", utf8(current.directory)}, {"session", session_id}});
  forget_capture_locked();
}

void refresh() noexcept {
  try {
    std::lock_guard guard(host_lock);
    if (configured)
      refresh_locked();
  } catch (...) {
    crash::disarm();
  }
}

void flush() {
  nlohmann::json request;
  {
    std::lock_guard guard(host_lock);
    if (!refresh_locked())
      return;
    request = bound_session_request();
  }
#if defined(MSIME_TELEMETRY_TEST_FLUSH)
  (void)call(msime_test_telemetry_flush, request);
#else
  (void)call(msime_client_telemetry_flush, request);
#endif
  // 旧网络请求的 enabled:false 不能解除新代次的武装；只按当前配置重新观察。
  refresh();
}

void start_flushing() {
  std::thread([] {
    for (;;) {
      flush();
      std::this_thread::sleep_for(std::chrono::minutes(30));
    }
  }).detach();
}

void set_enabled(bool enabled) {
  std::lock_guard guard(host_lock);
  if (!configured)
    return;
  // 产品宿主始终以持久偏好为准，迟到的发布回调不能撤回已重开的新代次。
  if (current.preferences_directory.empty())
    current.enabled = enabled;
  refresh_locked();
}

namespace {
void stop_after_observer_failure() noexcept {
  crash::disarm();
  try {
    std::lock_guard guard(host_lock);
    configured = false;
    forget_capture_locked();
  } catch (...) {
  }
}
} // namespace

struct ConsentObserver::State {
  std::mutex mutex;
  std::condition_variable wake;
  bool stop = false;
  std::thread worker;
};

ConsentObserver::ConsentObserver() noexcept {
  try {
    state_ = std::make_unique<State>();
    refresh();
    state_->worker = std::thread([state = state_.get()] {
      try {
        std::unique_lock lock(state->mutex);
        while (!state->wake.wait_for(lock, std::chrono::milliseconds(500), [state] { return state->stop; })) {
          lock.unlock();
          refresh();
          lock.lock();
        }
      } catch (...) {
        stop_after_observer_failure();
      }
    });
  } catch (...) {
    stop_after_observer_failure();
  }
}

ConsentObserver::~ConsentObserver() {
  if (state_) {
    {
      std::lock_guard guard(state_->mutex);
      state_->stop = true;
    }
    state_->wake.notify_all();
    if (state_->worker.joinable())
      state_->worker.join();
  }
  try {
    end();
  } catch (...) {
    crash::disarm();
  }
}

struct CaptureOnly::State {
  explicit State(ReadHost reader) : read_host(std::move(reader)) {}
  ReadHost read_host;
  std::mutex mutex;
  std::condition_variable wake;
  bool stop = false;
  std::thread worker;
  std::optional<Host> previous;
  bool cleared = false;
  std::string consent_epoch;
  std::string record_path;
  std::string record_header;
  std::terminate_handler previous_terminate = nullptr;
  bool terminate_installed = false;

  bool same_target(const Host &host) const {
    return previous && previous->directory == host.directory &&
           previous->preferences_directory == host.preferences_directory &&
           previous->platform == host.platform && previous->version == host.version &&
           previous->component == host.component && previous->edition == host.edition;
  }

  void suspend() {
    // 临时不可读只解除武装；同一持续授权恢复后沿用原身份。
    crash::disarm();
  }

  void forget() {
    suspend();
    previous.reset();
    consent_epoch.clear();
    record_path.clear();
    record_header.clear();
    cleared = false;
  }

  bool arm_authorized(const Host &host, const std::string &epoch) {
#if defined(MSIME_TELEMETRY_TEST_CAPTURE)
    msime_test_capture_before_arm();
#endif
    // 准备与恢复均可能跨过一次撤回；真正武装前只认仍开启的原代次。
    const auto consent = call(msime_client_telemetry_consent, consent_request(host));
    if (!consent || !consent->is_object() || !consent->contains("enabled") ||
        !(*consent)["enabled"].is_boolean() || !(*consent)["enabled"].get<bool>()) {
      suspend();
      return false;
    }
    const auto current_epoch = consent->find("epoch");
    if (current_epoch == consent->end() || !current_epoch->is_string() ||
        current_epoch->get_ref<const std::string &>() != epoch) {
      suspend();
      return false;
    }
    crash::arm(record_path, record_header);
    return crash::is_armed();
  }

  void refresh() noexcept {
    try {
      auto host = read_host();
      if (!host || !host->directory.is_absolute() ||
          !host->preferences_directory.is_absolute()) {
        suspend();
        return;
      }
      // 偏好读取会创建目录和锁。首次准备还未建立状态目录时，不能因此让独占创建失败。
      std::error_code error;
      if (!std::filesystem::is_directory(host->preferences_directory, error) || error) {
        suspend();
        return;
      }
      const auto consent = call(msime_client_telemetry_consent, consent_request(*host));
      if (!consent || !consent->is_object() || !consent->contains("enabled") ||
          !(*consent)["enabled"].is_boolean()) {
        suspend();
        return;
      }
      host->capture_only = true;
      // 持续授权必须由共享偏好核验，不能让缓存布尔值绕过代次。
      host->enabled.reset();
      if (!(*consent)["enabled"].get<bool>()) {
        crash::disarm();
        consent_epoch.clear();
        record_path.clear();
        record_header.clear();
        if (!same_target(*host) || !cleared) {
          const auto result = call(msime_client_telemetry_clear,
              nlohmann::json{{"directory", utf8(host->directory)},
                             {"preferences_directory", utf8(host->preferences_directory)}});
          // 同意读完后可能已重新开启；条件清理拒绝时不能记作成功。
          cleared = result && result->is_boolean() && result->get<bool>();
        }
        previous = std::move(host);
        return;
      }
      const auto epoch = consent->find("epoch");
      if (epoch == consent->end() || !epoch->is_string() || epoch->get_ref<const std::string &>().empty()) {
        suspend();
        return;
      }
      const auto authorized_epoch = epoch->get<std::string>();
      if (same_target(*host) && consent_epoch == authorized_epoch && !record_path.empty()) {
        if (!crash::is_armed())
          arm_authorized(*host, authorized_epoch);
        return;
      }
      forget();
      auto request = session_request(*host);
      request["consent_epoch"] = authorized_epoch;
      const auto value = call(msime_client_telemetry_begin, request);
      if (!value || !value->is_object() || !value->value("enabled", false))
        return;
      const auto path = value->find("crash_record_path");
      const auto header = value->find("crash_record_header");
      if (path == value->end() || !path->is_string() ||
          header == value->end() || !header->is_string())
        return;
      record_path = path->get<std::string>();
      record_header = header->get<std::string>();
      if (arm_authorized(*host, authorized_epoch)) {
        consent_epoch = authorized_epoch;
        previous = std::move(host);
      }
    } catch (...) {
      // 不可读、损坏或分配失败均解除采集，不能用默认同意追认未知状态。
      suspend();
    }
  }

  void run() {
    std::unique_lock lock(mutex);
    while (!wake.wait_for(lock, std::chrono::milliseconds(500), [this] { return stop; })) {
      lock.unlock();
      refresh();
#if defined(MSIME_TELEMETRY_TEST_CAPTURE)
      msime_test_capture_after_refresh();
#endif
      lock.lock();
    }
  }
};

CaptureOnly::CaptureOnly(ReadHost read_host) noexcept {
  try {
    state_ = std::make_unique<State>(std::move(read_host));
    // 在主线程安装备用栈；故障时只使用预备好的原生记录器。
    crash::install_crash_handlers();
    state_->previous_terminate = std::set_terminate(auxiliary_terminate);
    auxiliary_previous_terminate = state_->previous_terminate;
    state_->terminate_installed = true;
    state_->refresh();
    state_->worker = std::thread([state = state_.get()] {
      try {
        state->run();
      } catch (...) {
        // 无法继续观察同意时解除采集，不能留下永久启用的记录器。
        crash::disarm();
      }
    });
  } catch (...) {
    crash::disarm();
  }
}

CaptureOnly::~CaptureOnly() {
  if (state_) {
    {
      std::lock_guard guard(state_->mutex);
      state_->stop = true;
    }
    state_->wake.notify_all();
    if (state_->worker.joinable())
      state_->worker.join();
  }
  // 退出作用域也可能只是被上层捕获的异常；观察线程结束后不能留下不会再响应同意的记录器。
  crash::remove_crash_handlers();
  if (state_ && state_->terminate_installed && std::get_terminate() == auxiliary_terminate)
    std::set_terminate(state_->previous_terminate);
}

std::string current_exception_summary() {
  const auto exception = std::current_exception();
  if (!exception)
    return "std::terminate without an active exception";
  try {
    std::rethrow_exception(exception);
  } catch (const nlohmann::json::exception &error) {
    // A JSON error's text quotes the bytes it failed on, which can come from the user's own files; the type and id say where it failed without them.
    return "std::terminate: " + demangle(typeid(error).name()) + " " + std::to_string(error.id);
  } catch (const std::exception &error) {
    // what() 可能包含输入、路径或凭据，只保留异常类型。
    return "std::terminate: " + demangle(typeid(error).name());
  } catch (...) {
    return "std::terminate: unknown exception";
  }
}

void record_exception(const std::string &summary, const std::string &stack) {
  try {
    if (!crash::is_armed())
      return;
    std::filesystem::path directory;
    std::string session;
    std::string header;
    {
      // try_lock: the thread that holds the lock may be the one terminating.
      std::unique_lock guard(host_lock, std::try_to_lock);
      if (!guard.owns_lock() || !configured || session_id.empty())
        return;
      directory = current.directory;
      session = session_id;
      header = capture_header;
    }
    // 语言异常与后续致命信号保留同一份记录；身份取本进程快照，不按可能属于另一进程的标记找会话。
    call(msime_client_telemetry_record_crash, nlohmann::json{{"directory", utf8(directory)},
                                                            {"message", summary},
                                                            {"stack", stack},
                                                            {"session", session}, {"crash_record_header", header}});
  } catch (...) {
  }
}

void record_terminate() {
  try {
    if (!crash::is_armed()) return;
    record_exception(current_exception_summary(), current_stack(1));
  } catch (...) {
  }
}

std::string current_stack(int skip) { return crash::current_stack(skip + 1); }
void install_crash_handlers() { crash::install_crash_handlers(); }
void remove_crash_handlers() { crash::remove_crash_handlers(); }
}
