#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include "TsfCrashCapture.h"
#include "CrashCaptureContext.h"
#include "HostOptionsPaths.h"
#include "HostPathEncoding.h"
#include "msime_tsf_version.h"
#include "../../../shared/contracts/msime_edition.h"
#include "../../common/CrashRecorder.h"
#include "../../common/HostApiString.h"
#include <windows.h>
#include <werapi.h>
#include <nlohmann/json.hpp>
#include <cstring>
#include <filesystem>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
// 仅专项替代系统注册；租约、线程池、加载引用、授权请求与快照发布都运行生产代码。
extern "C" HRESULT msime_test_wer_register(PCWSTR, PVOID);
extern "C" HRESULT msime_test_wer_unregister(PCWSTR, PVOID);
extern "C" void msime_test_tsf_after_refresh();
extern "C" void msime_test_tsf_before_publish();
#endif

namespace msime::tsf {
namespace {
using wer::Context;
using wer::Module;
using Request = char *(*)(const uint8_t *, size_t);

struct State {
  Context context{};
  HANDLE wake = nullptr;
  HMODULE module = nullptr;
  bool callback_owns_module = false;
  unsigned leases = 1;
  std::uint64_t generation = 1;
  bool registered = false;
  std::wstring helper;
  std::string preferences;
  std::filesystem::path directory;
  std::string epoch;
  std::string path;
  std::string header;
  std::uint64_t prepared_generation = 0;
  bool cleared = false;
  ~State() {
    if (wake) CloseHandle(wake);
    if (module && !callback_owns_module) FreeLibrary(module);
  }
};
// 不注册静态析构：后台任务结束并注销 WER 后才销毁堆状态，不在 loader lock 内等待。
SRWLOCK state_lock = SRWLOCK_INIT;
State *current_state = nullptr;
class Guard {
public:
  Guard() { AcquireSRWLockExclusive(&state_lock); }
  ~Guard() { ReleaseSRWLockExclusive(&state_lock); }
  Guard(const Guard &) = delete;
  Guard &operator=(const Guard &) = delete;
};

void disarm(State &state) noexcept {
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.armed), 0);
}
std::optional<nlohmann::json> call(Request function, const nlohmann::json &request) {
  const auto body = request.dump();
  auto raw = msime::host_api::own_string(function(reinterpret_cast<const uint8_t *>(body.data()), body.size()));
  if (!raw) return std::nullopt;
  auto envelope = nlohmann::json::parse(raw.get(), nullptr, false);
  if (!envelope.is_object() || !envelope.contains("ok") || !envelope["ok"].is_boolean() ||
      !envelope["ok"].get<bool>() || !envelope.contains("value")) return std::nullopt;
  return std::move(envelope["value"]);
}
bool boolean(const nlohmann::json &object, const char *name, bool expected) {
  const auto found = object.find(name);
  return object.is_object() && found != object.end() && found->is_boolean() && found->get<bool>() == expected;
}
std::filesystem::path telemetry_directory() {
  std::vector<wchar_t> value(32768);
  const DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA", value.data(), static_cast<DWORD>(value.size()));
  if (!length || length >= value.size()) return {};
  const std::filesystem::path local(std::wstring(value.data(), length));
  return local.is_absolute() ? local / MSIME_EDITION_USER_DATA_DIRECTORY : std::filesystem::path{};
}
std::wstring module_path(HMODULE handle) {
  std::vector<wchar_t> value(32768);
  const DWORD length = GetModuleFileNameW(handle, value.data(), static_cast<DWORD>(value.size()));
  return length && length < value.size() ? std::wstring(value.data(), length) : std::wstring{};
}
bool describe(Module &out, HMODULE handle, const std::string &name) {
  if (!handle || name.empty() || name.size() >= sizeof(out.name)) return false;
  // 只读取由加载器验证且仍由本 DLL 导入引用持有的 PE，不扫描宿主的任意模块。
  const auto *dos = reinterpret_cast<const IMAGE_DOS_HEADER *>(handle);
  if (dos->e_magic != IMAGE_DOS_SIGNATURE) return false;
  const auto *nt = reinterpret_cast<const IMAGE_NT_HEADERS *>(reinterpret_cast<const BYTE *>(handle) + dos->e_lfanew);
  if (nt->Signature != IMAGE_NT_SIGNATURE || !nt->OptionalHeader.SizeOfImage) return false;
  out.base = reinterpret_cast<std::uintptr_t>(handle);
  out.size = nt->OptionalHeader.SizeOfImage;
  std::memcpy(out.name, name.c_str(), name.size() + 1);
  return true;
}
bool modules(State &state, Context &next) {
  std::filesystem::path host_name(MSIME_EDITION_HOST_DLL);
#if defined(MSIME_TSF_NATIVE_ARM64_HOST)
  host_name = host_name.stem().wstring() + L"_arm64.dll";
#endif
  const HMODULE host = GetModuleHandleW(host_name.c_str());
  // 导入库可能经过 TIP 的跳板，不能用函数的本地跳板地址冒充 Host API 模块范围。
  if (!host || host == state.module || !GetProcAddress(host, "msime_client_telemetry_begin")) return false;
  return describe(next.modules[0], state.module, "MetasequoiaImeTsf.dll") &&
      describe(next.modules[1], host, path_to_utf8(host_name));
}

HRESULT register_helper(State &state) {
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
  return msime_test_wer_register(state.helper.c_str(), &state.context);
#else
  return WerRegisterRuntimeExceptionModule(state.helper.c_str(), &state.context);
#endif
}
HRESULT unregister_helper(State &state) {
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
  return msime_test_wer_unregister(state.helper.c_str(), &state.context);
#else
  return WerUnregisterRuntimeExceptionModule(state.helper.c_str(), &state.context);
#endif
}

// 水杉自己的设置、面板等进程装了进程内记录器，它照样把故障交给 WER；能到达未处理异常过滤器的故障助手再记就是第二份，还会记成 TSF 的。__fastfail、/GS、CFG 和堆损坏绕过过滤器，只有助手能记，所以照常注册武装，只在快照里标明。
std::uint32_t in_process_recorder() {
  wchar_t value[16]{};
  const DWORD length = GetEnvironmentVariableW(msime::crash::in_process_recorder_variable, value, 16);
  return length && length < 16 && std::wstring(value, length) == std::to_wstring(GetCurrentProcessId()) ? 1u : 0u;
}

bool publish(State &state, std::uint64_t generation) {
  const auto wide = std::filesystem::u8path(state.path).wstring();
  if (wide.empty() || wide.size() >= wer::path_capacity || state.header.empty() ||
      state.header.size() >= wer::header_capacity) return false;
  Context next{};
  std::memcpy(next.path, wide.c_str(), (wide.size() + 1) * sizeof(wchar_t));
  std::memcpy(next.header, state.header.c_str(), state.header.size() + 1);
  next.in_process_recorder = in_process_recorder();
  if (!modules(state, next)) return false;
  Guard guard;
  // 最后释放及重新激活都会换代；在文件锁中迟到的准备不能重新武装旧租约。
  if (current_state != &state || !state.leases || state.generation != generation) return false;
  if (!state.registered) {
    if (FAILED(register_helper(state))) return false;
    state.registered = true;
  }
  disarm(state);
  auto sequence = state.context.sequence + std::uint32_t{2};
  if (!sequence) sequence = 2;
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.sequence), static_cast<LONG>(sequence - 1));
  std::memcpy(state.context.path, next.path, sizeof(next.path));
  std::memcpy(state.context.header, next.header, sizeof(next.header));
  std::memcpy(state.context.modules, next.modules, sizeof(next.modules));
  state.context.in_process_recorder = next.in_process_recorder;
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.sequence), static_cast<LONG>(sequence));
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.armed), 1);
  return true;
}

void forget(State &state);

bool still_authorized(const State &state, const std::string &epoch) {
  const auto consent = call(msime_client_telemetry_consent,
                            {{"directory", state.preferences}});
  if (!consent || !consent->is_object() || !boolean(*consent, "enabled", true)) return false;
  const auto found = consent->find("epoch");
  return found != consent->end() && found->is_string() && found->get_ref<const std::string &>() == epoch;
}

bool publish_authorized(State &state, std::uint64_t generation, const std::string &epoch) {
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
  // 让专项在 begin 回包和最终授权复核之间注入撤回/换代，覆盖真实竞态窗口。
  msime_test_tsf_before_publish();
#endif
  if (!still_authorized(state, epoch)) {
    forget(state);
    return false;
  }
  return publish(state, generation);
}

void forget(State &state) {
  disarm(state);
  state.epoch.clear();
  state.path.clear();
  state.header.clear();
}
bool refresh(State &state, std::uint64_t generation, const std::string &preferences) {
  const auto directory = telemetry_directory();
  if (preferences.empty() || !std::filesystem::u8path(preferences).is_absolute() || directory.empty()) {
    disarm(state); return false;
  }
  std::error_code error;
  if (!std::filesystem::is_directory(std::filesystem::u8path(preferences), error) || error) {
    disarm(state); return false;
  }
  if (state.preferences != preferences || state.directory != directory || state.prepared_generation != generation) {
    forget(state);
    state.preferences = preferences;
    state.directory = directory;
    state.prepared_generation = generation;
    state.cleared = false;
  }
  const auto consent = call(msime_client_telemetry_consent, {{"directory", preferences}});
  if (!consent || !consent->is_object() || !consent->contains("enabled") || !(*consent)["enabled"].is_boolean()) {
    disarm(state); return false;
  }
  if (boolean(*consent, "enabled", false)) {
    forget(state);
    if (!state.cleared) {
      const auto cleared = call(msime_client_telemetry_clear,
          {{"directory", path_to_utf8(directory)}, {"preferences_directory", preferences}});
      state.cleared = cleared && cleared->is_boolean() && cleared->get<bool>();
    }
    return state.cleared;
  }
  const auto epoch = consent->find("epoch");
  if (epoch == consent->end() || !epoch->is_string() || epoch->get_ref<const std::string &>().empty()) {
    disarm(state); return false;
  }
  const auto authorized = epoch->get<std::string>();
  if (state.epoch != authorized || state.path.empty()) {
    forget(state);
    const auto response = call(msime_client_telemetry_begin,
        {{"directory", path_to_utf8(directory)}, {"preferences_directory", preferences},
         {"platform", "windows"},
         {"version", std::to_string(MSIME_TSF_VERSION_MAJOR) + "." + std::to_string(MSIME_TSF_VERSION_MINOR) + "." + std::to_string(MSIME_TSF_VERSION_PATCH)},
         {"component", "tsf"}, {"capture_only", true}, {"consent_epoch", authorized}});
    if (!response || !boolean(*response, "enabled", true)) return false;
    const auto path = response->find("crash_record_path");
    const auto header = response->find("crash_record_header");
    if (path == response->end() || !path->is_string() || header == response->end() || !header->is_string()) return false;
    state.path = path->get<std::string>();
    state.header = header->get<std::string>();
    state.epoch = authorized;
    state.prepared_generation = generation;
    state.cleared = false;
  }
  // 记录器可能在 TIP 之后才装上或卸下，每轮重新核对。
  const auto recorder = in_process_recorder();
  {
    Guard guard;
    if (current_state != &state || !state.leases || state.generation != generation) {
      disarm(state);
      return false;
    }
    // 同代次已武装且记录器标记没变时保留快照；首次准备、暂停恢复和标记变化仍走发布前的最终授权复核。
    if (InterlockedCompareExchange(reinterpret_cast<volatile LONG *>(&state.context.armed), 0, 0) == 1 &&
        state.context.in_process_recorder == recorder)
      return true;
  }
  if (publish_authorized(state, generation, authorized)) return true;
  disarm(state);
  return false;
}

class PreferenceChanges {
public:
  ~PreferenceChanges() { reset(); }
  void reset() noexcept {
    close(directory_);
    close(parent_);
  }
  bool prepare(const std::string &preferences) {
    const auto path = std::filesystem::u8path(preferences);
    if (path != path_) {
      reset();
      path_ = path;
    }
    // 远程文件系统可能不投递变化通知，继续使用快速读取兜底。
    if (!path.is_absolute() || GetDriveTypeW(path.root_path().c_str()) == DRIVE_REMOTE) return false;
    // 必须先订阅再读取授权，不能丢失首次检查期间的撤回。
    if (parent_ == INVALID_HANDLE_VALUE)
      parent_ = FindFirstChangeNotificationW(path.parent_path().c_str(), FALSE,
          FILE_NOTIFY_CHANGE_DIR_NAME | FILE_NOTIFY_CHANGE_SECURITY);
    if (directory_ == INVALID_HANDLE_VALUE)
      directory_ = FindFirstChangeNotificationW(path.c_str(), FALSE,
          FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_SIZE |
          FILE_NOTIFY_CHANGE_ATTRIBUTES | FILE_NOTIFY_CHANGE_SECURITY);
    return parent_ != INVALID_HANDLE_VALUE && directory_ != INVALID_HANDLE_VALUE;
  }
  bool wait(HANDLE wake, DWORD timeout) noexcept {
    HANDLE handles[3]{wake};
    DWORD count = 1;
    if (parent_ != INVALID_HANDLE_VALUE) handles[count++] = parent_;
    if (directory_ != INVALID_HANDLE_VALUE) handles[count++] = directory_;
    const DWORD result = WaitForMultipleObjects(count, handles, FALSE, timeout);
    if (result == WAIT_OBJECT_0) return true;
    if (result > WAIT_OBJECT_0 && result < WAIT_OBJECT_0 + count) {
      const HANDLE signaled = handles[result - WAIT_OBJECT_0];
      if (signaled == parent_) {
        // 子目录可能已被移走；旧句柄仍观察旧目录，下一轮必须按当前路径重建。
        close(directory_);
        if (!FindNextChangeNotification(parent_)) close(parent_);
      } else if (!FindNextChangeNotification(directory_)) {
        close(directory_);
      }
      return true;
    }
    if (result == WAIT_TIMEOUT) {
      // 低频兜底同时重新解析父路径，覆盖更上层目录整体替换。
      if (timeout == 30000) reset();
      return true;
    }
    reset();
    return false;
  }
private:
  static void close(HANDLE &handle) noexcept {
    if (handle != INVALID_HANDLE_VALUE) FindCloseChangeNotification(handle);
    handle = INVALID_HANDLE_VALUE;
  }
  std::filesystem::path path_;
  HANDLE directory_ = INVALID_HANDLE_VALUE, parent_ = INVALID_HANDLE_VALUE;
};

void CALLBACK observe(PTP_CALLBACK_INSTANCE instance, void *context) noexcept {
  std::unique_ptr<State> owner(static_cast<State *>(context));
  State &state = *owner;
  state.callback_owns_module = true;
  FreeLibraryWhenCallbackReturns(instance, state.module);
  CallbackMayRunLong(instance);
  PreferenceChanges changes;
  for (;;) {
    std::uint64_t generation = 0;
    bool finished = false;
    {
      Guard guard;
      if (!state.leases) {
        disarm(state);
        // 注销失败时保留上下文和加载引用，避免系统稍后读取已释放地址。
        if (!state.registered || SUCCEEDED(unregister_helper(state))) {
          current_state = nullptr;
          finished = true;
        }
      } else {
        generation = state.generation;
      }
      ResetEvent(state.wake);
    }
    if (finished) break;
    bool stable = false;
    if (generation) {
      try {
        const auto preferences = default_state_directory();
        const bool watching = changes.prepare(preferences);
        stable = refresh(state, generation, preferences) && watching;
      }
      catch (...) { disarm(state); }
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
      msime_test_tsf_after_refresh();
#endif
    }
    if (!changes.wait(state.wake, stable ? 30000 : 500)) disarm(state);
  }
}

bool acquire() noexcept {
  try {
    Guard guard;
    if (current_state) {
      if (current_state->leases == MAXDWORD) return false;
      if (!current_state->leases) ++current_state->generation;
      ++current_state->leases;
      SetEvent(current_state->wake);
      return true;
    }
    auto next = std::make_unique<State>();
    // 只准备内存与内核句柄，文件、偏好和 Rust 调用都留给后台任务。
    next->wake = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    if (!next->wake) return false;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                            reinterpret_cast<LPCWSTR>(&observe), &next->module)) {
      return false;
    }
    const auto path = module_path(next->module);
    if (path.empty()) {
      return false;
    }
    next->helper = (std::filesystem::path(path).parent_path() / L"MetasequoiaImeCrashWer.dll").wstring();
    current_state = next.get();
    if (!TrySubmitThreadpoolCallback(observe, next.get(), nullptr)) {
      current_state = nullptr;
      return false;
    }
    next.release();
    return true;
  } catch (...) { return false; }
}
void release() noexcept {
  Guard guard;
  if (!current_state || !current_state->leases || --current_state->leases) return;
  ++current_state->generation;
  disarm(*current_state);
  SetEvent(current_state->wake);
}
} // namespace

TsfCrashCapture::TsfCrashCapture() noexcept : lease_(acquire()) {}
TsfCrashCapture::~TsfCrashCapture() noexcept { if (lease_) release(); }
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
// 测试只能在生命周期锁内复制快照，不能持有可能被后台任务释放的上下文指针。
extern "C" __declspec(dllexport) bool msime_test_tsf_snapshot(wer::Context *out) noexcept {
  Guard guard;
  if (!current_state) return false;
  *out = current_state->context;
  return true;
}
#endif
} // namespace msime::tsf
