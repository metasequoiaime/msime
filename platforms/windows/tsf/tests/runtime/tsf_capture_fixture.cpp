#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <werapi.h>
#include "../../TsfCrashCapture.h"
#include "../../HostPathEncoding.h"
#include "../../CrashCaptureContext.h"
#include <atomic>

namespace {
std::atomic<unsigned> registered{0}, unregistered{0}, failures{0};
// 生产查询交来的真实 WOW64 结果，-1 表示本次加载还没查过。
std::atomic<int> wow64_actual{-1};
bool system_wer() {
  wchar_t value[2]{};
  return GetEnvironmentVariableW(L"MSIME_TSF_TEST_SYSTEM_WER", value, 2) == 1 && value[0] == L'1';
}
struct RefreshBarrier {
  HANDLE entered = nullptr, resumed = nullptr;
  std::atomic<bool> blocked{false};
  ~RefreshBarrier() { if (entered) CloseHandle(entered); if (resumed) CloseHandle(resumed); }
} refresh_barrier;
RefreshBarrier publish_barrier;
}
namespace msime::tsf {
// 路径发现边界用合成目录；后台租约、共享 ABI、线程池和加载引用使用生产实现。
std::string default_state_directory() {
  wchar_t path[1024]{};
  const auto length = GetEnvironmentVariableW(L"MSIME_TSF_TEST_PREFERENCES", path, 1024);
  return length && length < 1024 ? path_to_utf8(std::filesystem::path(path)) : std::string{};
}
}
extern "C" HRESULT msime_test_wer_register(PCWSTR path, PVOID context) {
  if (!path || !context || std::filesystem::path(path).filename() != L"MetasequoiaImeCrashWer.dll") return E_INVALIDARG;
  ++registered;
  if (system_wer()) return WerRegisterRuntimeExceptionModule(path, context);
  return S_OK;
}
// MSIME_TSF_TEST_WOW64 为 1 或 0 时改写成假装 WOW64 或非 WOW64，未设置时照用系统结果；查询在每次加载 DLL 后的首个租约发生一次。
extern "C" BOOL msime_test_wow64_process(BOOL actual) {
  wow64_actual = actual ? 1 : 0;
  wchar_t value[2]{};
  if (GetEnvironmentVariableW(L"MSIME_TSF_TEST_WOW64", value, 2) == 1 && (value[0] == L'0' || value[0] == L'1'))
    return value[0] == L'1';
  return actual;
}
extern "C" HRESULT msime_test_wer_unregister(PCWSTR path, PVOID context) {
  ++unregistered;
  if (system_wer()) return WerUnregisterRuntimeExceptionModule(path, context);
  auto count = failures.load();
  while (count && !failures.compare_exchange_weak(count, count - 1)) {}
  return count ? E_FAIL : S_OK;
}
extern "C" void msime_test_tsf_after_refresh() {
  if (refresh_barrier.blocked.exchange(false)) {
    SetEvent(refresh_barrier.entered);
    WaitForSingleObject(refresh_barrier.resumed, 10000);
  }
}
extern "C" void msime_test_tsf_before_publish() {
  if (publish_barrier.blocked.exchange(false)) {
    SetEvent(publish_barrier.entered);
    WaitForSingleObject(publish_barrier.resumed, 10000);
  }
}
extern "C" {
__declspec(noinline) void msime_test_tsf_crash() {
  // 仅在显式系统专项中触发真实故障，验证 WER 的首帧归属。
  volatile std::uintptr_t address = 1;
  *reinterpret_cast<volatile unsigned *>(address) = 42;
}
void *msime_test_tsf_acquire() { return new msime::tsf::TsfCrashCapture(); }
void msime_test_tsf_release(void *lease) { delete static_cast<msime::tsf::TsfCrashCapture *>(lease); }
unsigned msime_test_tsf_registered() { return registered.load(); }
unsigned msime_test_tsf_unregistered() { return unregistered.load(); }
int msime_test_tsf_wow64_actual() { return wow64_actual.load(); }
void msime_test_tsf_fail_unregister(unsigned count) { failures = count; }
void msime_test_tsf_block_refresh() {
  if (!refresh_barrier.entered) refresh_barrier.entered = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (!refresh_barrier.resumed) refresh_barrier.resumed = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  ResetEvent(refresh_barrier.entered);
  ResetEvent(refresh_barrier.resumed);
  refresh_barrier.blocked = true;
}
bool msime_test_tsf_wait_refresh(DWORD timeout) { return WaitForSingleObject(refresh_barrier.entered, timeout) == WAIT_OBJECT_0; }
void msime_test_tsf_resume_refresh() { SetEvent(refresh_barrier.resumed); }
void msime_test_tsf_block_publish() {
  if (!publish_barrier.entered) publish_barrier.entered = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (!publish_barrier.resumed) publish_barrier.resumed = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  ResetEvent(publish_barrier.entered);
  ResetEvent(publish_barrier.resumed);
  publish_barrier.blocked = true;
}
bool msime_test_tsf_wait_publish(DWORD timeout) {
  return WaitForSingleObject(publish_barrier.entered, timeout) == WAIT_OBJECT_0;
}
void msime_test_tsf_resume_publish() { SetEvent(publish_barrier.resumed); }
}
