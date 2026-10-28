#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include "TsfCrashCapture.h"
#include "CrashCaptureContext.h"
#include "HostOptionsPaths.h"
#include "HostPathEncoding.h"
#include "msime_tsf_version.h"
#include "../../../shared/contracts/msime_edition.h"
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

bool publish(State &state, std::uint64_t generation) {
  const auto wide = std::filesystem::u8path(state.path).wstring();
  if (wide.empty() || wide.size() >= wer::path_capacity || state.header.empty() ||
      state.header.size() >= wer::header_capacity) return false;
  Context next{};
  std::memcpy(next.path, wide.c_str(), (wide.size() + 1) * sizeof(wchar_t));
  std::memcpy(next.header, state.header.c_str(), state.header.size() + 1);
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
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.sequence), static_cast<LONG>(sequence));
  InterlockedExchange(reinterpret_cast<volatile LONG *>(&state.context.armed), 1);
  return true;
}

void forget(State &state) {
  disarm(state);
  state.epoch.clear();
  state.path.clear();
  state.header.clear();
}
void refresh(State &state, std::uint64_t generation) {
  const auto preferences = default_state_directory();
  const auto directory = telemetry_directory();
  if (preferences.empty() || !std::filesystem::u8path(preferences).is_absolute() || directory.empty()) {
    disarm(state); return;
  }
  std::error_code error;
  if (!std::filesystem::is_directory(std::filesystem::u8path(preferences), error) || error) {
    disarm(state); return;
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
    disarm(state); return;
  }
  if (boolean(*consent, "enabled", false)) {
    forget(state);
    if (!state.cleared) {
      const auto cleared = call(msime_client_telemetry_clear,
          {{"directory", path_to_utf8(directory)}, {"preferences_directory", preferences}});
      state.cleared = cleared && cleared->is_boolean() && cleared->get<bool>();
    }
    return;
  }
  const auto epoch = consent->find("epoch");
  if (epoch == consent->end() || !epoch->is_string() || epoch->get_ref<const std::string &>().empty()) {
    disarm(state); return;
  }
  const auto authorized = epoch->get<std::string>();
  if (state.epoch != authorized || state.path.empty()) {
    forget(state);
    const auto response = call(msime_client_telemetry_begin,
        {{"directory", path_to_utf8(directory)}, {"preferences_directory", preferences},
         {"platform", "windows"},
         {"version", std::to_string(MSIME_TSF_VERSION_MAJOR) + "." + std::to_string(MSIME_TSF_VERSION_MINOR) + "." + std::to_string(MSIME_TSF_VERSION_PATCH)},
         {"component", "tsf"}, {"capture_only", true}, {"consent_epoch", authorized}});
    if (!response || !boolean(*response, "enabled", true)) return;
    const auto path = response->find("crash_record_path");
    const auto header = response->find("crash_record_header");
    if (path == response->end() || !path->is_string() || header == response->end() || !header->is_string()) return;
    state.path = path->get<std::string>();
    state.header = header->get<std::string>();
    state.epoch = authorized;
    state.prepared_generation = generation;
    state.cleared = false;
  }
  if (!publish(state, generation)) disarm(state);
}

void CALLBACK observe(PTP_CALLBACK_INSTANCE instance, void *context) noexcept {
  std::unique_ptr<State> owner(static_cast<State *>(context));
  State &state = *owner;
  state.callback_owns_module = true;
  FreeLibraryWhenCallbackReturns(instance, state.module);
  CallbackMayRunLong(instance);
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
    if (generation) {
      try { refresh(state, generation); }
      catch (...) { disarm(state); }
#if defined(MSIME_TSF_CRASH_CAPTURE_TEST)
      msime_test_tsf_after_refresh();
#endif
    }
    WaitForSingleObject(state.wake, 500);
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
