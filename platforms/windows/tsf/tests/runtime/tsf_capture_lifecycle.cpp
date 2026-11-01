#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <werapi.h>
#include "../../CrashCaptureContext.h"
#include "../../../../common/CrashRecorder.h"
#include "../../WerApiCompatibility.h"
#include "../../../../../shared/contracts/msime_edition.h"
#include <chrono>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>

namespace {
using msime::tsf::wer::Context;
void require(bool value, const char *message) { if (!value) throw std::runtime_error(message); }
template<class Function> Function load(HMODULE module, const char *name) {
  const auto raw = GetProcAddress(module, name);
  require(raw != nullptr, name);
  Function function = nullptr;
  static_assert(sizeof(function) == sizeof(raw));
  std::memcpy(&function, &raw, sizeof(function));
  return function;
}
template<class Predicate> void until(Predicate predicate, const char *message) {
  const auto end = GetTickCount64() + 6000;
  do { if (predicate()) return; Sleep(10); } while (GetTickCount64() < end);
  throw std::runtime_error(message);
}
struct Tip {
  HMODULE module = nullptr;
  void *(*acquire)() = nullptr;
  void (*release)(void *) = nullptr;
  bool (*snapshot)(Context *) = nullptr;
  unsigned (*registered)() = nullptr;
  unsigned (*unregistered)() = nullptr;
  void (*fail_unregister)(unsigned) = nullptr;
  void (*block_refresh)() = nullptr;
  bool (*wait_refresh)(DWORD) = nullptr;
  void (*resume_refresh)() = nullptr;
  void (*block_publish)() = nullptr;
  bool (*wait_publish)(DWORD) = nullptr;
  void (*resume_publish)() = nullptr;
  explicit Tip(const std::filesystem::path &directory) {
    module = LoadLibraryW((directory / L"MetasequoiaImeTsfLeaseTest.dll").c_str());
    require(module != nullptr, "TIP fixture load failed");
    acquire = load<decltype(acquire)>(module, "msime_test_tsf_acquire");
    release = load<decltype(release)>(module, "msime_test_tsf_release");
    snapshot = load<decltype(snapshot)>(module, "msime_test_tsf_snapshot");
    registered = load<decltype(registered)>(module, "msime_test_tsf_registered");
    unregistered = load<decltype(unregistered)>(module, "msime_test_tsf_unregistered");
    fail_unregister = load<decltype(fail_unregister)>(module, "msime_test_tsf_fail_unregister");
    block_refresh = load<decltype(block_refresh)>(module, "msime_test_tsf_block_refresh");
    wait_refresh = load<decltype(wait_refresh)>(module, "msime_test_tsf_wait_refresh");
    resume_refresh = load<decltype(resume_refresh)>(module, "msime_test_tsf_resume_refresh");
    block_publish = load<decltype(block_publish)>(module, "msime_test_tsf_block_publish");
    wait_publish = load<decltype(wait_publish)>(module, "msime_test_tsf_wait_publish");
    resume_publish = load<decltype(resume_publish)>(module, "msime_test_tsf_resume_publish");
  }
  ~Tip() { if (module) FreeLibrary(module); }
  Context armed() {
    Context result{};
    until([&] { return snapshot(&result) && result.armed == 1; }, "TIP not armed");
    return result;
  }
  void disarmed() {
    until([&] { Context value{}; return !snapshot(&value) || value.armed == 0; }, "TIP not disarmed");
  }
  void unload() { require(FreeLibrary(module) != FALSE, "TIP release failed"); module = nullptr; }
};
void tests() {
  wchar_t buffer[32768]{};
  const auto length = GetModuleFileNameW(nullptr, buffer, 32768);
  require(length && length < 32768, "executable path missing");
  const auto directory = std::filesystem::path(buffer).parent_path();
  require(GetTempPathW(32768, buffer) != 0, "temporary path missing");
  const auto temporary = std::filesystem::path(buffer) / (L"msime-tsf-leases-" + std::to_wstring(GetCurrentProcessId()));
  require(std::filesystem::create_directory(temporary), "synthetic preferences directory collision");
  struct Cleanup {
    std::filesystem::path path;
    ~Cleanup() { std::error_code error; std::filesystem::remove_all(path, error); }
  } cleanup{temporary};
  const auto preferences = temporary / "preferences";
  require(std::filesystem::create_directory(preferences), "synthetic preferences creation failed");
  require(SetEnvironmentVariableW(L"LOCALAPPDATA", temporary.c_str()) != FALSE &&
          SetEnvironmentVariableW(L"MSIME_TSF_TEST_PREFERENCES", preferences.c_str()) != FALSE, "synthetic environment failed");
  const HMODULE host = LoadLibraryW((directory / MSIME_EDITION_HOST_DLL).c_str());
  require(host != nullptr, "Host API boundary load failed");
  const auto initialize = load<void (*)(const char *)>(host, "msime_test_host_initialize");
  const auto finish = load<void (*)()>(host, "msime_test_host_finish");
  const auto mode = load<void (*)(int)>(host, "msime_test_host_mode");
  const auto block = load<void (*)(int)>(host, "msime_test_host_block");
  const auto entered = load<bool (*)(DWORD)>(host, "msime_test_host_wait");
  const auto resume = load<void (*)()>(host, "msime_test_host_resume");
  const auto preparations = load<unsigned (*)()>(host, "msime_test_host_preparations");
  const auto invalid = load<unsigned (*)()>(host, "msime_test_host_invalid");
  const auto consent_reads = load<unsigned (*)()>(host, "msime_test_host_consent_reads");
  const auto root = temporary.u8string();
  initialize(root.c_str());
  const HMODULE helper = LoadLibraryW((directory / L"MetasequoiaImeCrashWer.dll").c_str());
  require(helper != nullptr, "production WER helper load failed");
  const auto record = load<msime::tsf::wer::ExceptionEvent>(helper, "OutOfProcessExceptionEventCallback");
  {
    Tip tip(directory);
    auto *first = tip.acquire();
    const auto initial = tip.armed();
    // 固定下一轮的首次同意读取与刷新结束，不能靠睡眠猜测是否完整执行了一轮。
    block(1);
    require(entered(6000), "steady consent barrier not reached");
    const auto reads_before = consent_reads();
    tip.block_refresh();
    resume();
    require(tip.wait_refresh(6000), "steady refresh checkpoint not reached");
    require(consent_reads() == reads_before + 1, "steady refresh reads consent more than once");
    require(tip.armed().sequence == initial.sequence && preparations() == 1,
            "steady refresh republished the same capture");
    tip.resume_refresh();
    // 文件通知可能合并或重复，先等未消费事件安静，再跨过三个旧轮询周期。
    unsigned quiet_reads = 0;
    until([&] {
      quiet_reads = consent_reads();
      Sleep(200);
      return consent_reads() == quiet_reads;
    }, "preference notifications did not settle");
    Sleep(1600);
    require(consent_reads() == quiet_reads, "idle observer still polls consent every 500 ms");
    WER_RUNTIME_EXCEPTION_INFORMATION information{};
    information.dwSize = sizeof(information);
    information.hProcess = GetCurrentProcess();
    information.hThread = GetCurrentThread();
    information.bIsFatal = TRUE;
    information.exceptionRecord.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
    information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(initial.modules[0].base + 0x100);
    // 真实采集准备交给生产 WER helper，不能让生命周期测试给出 helper 无法消费的路径或头。
    Context snapshot{};
    require(tip.snapshot(&snapshot), "prepared context unavailable");
    BOOL ownership = TRUE;
    require(record(&snapshot, &information, &ownership, nullptr, nullptr, nullptr) == S_OK && !ownership,
            "capture integration changed host WER disposition");
    const auto path = std::filesystem::path(std::u16string(snapshot.path));
    std::ifstream stream(path, std::ios::binary);
    const std::string saved{std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>()};
    require(saved.find(initial.header) == 0 && saved.find("MetasequoiaImeTsf.dll+0x100") != std::string::npos,
            "prepared TSF snapshot could not produce a WER record");
    require(tip.registered() == 1, "first lease did not register exactly once");
    auto *second = tip.acquire();
    tip.release(first);
    require(tip.armed().sequence != 0 && tip.registered() == 1, "one instance removed another lease");
    mode(2); tip.disarmed();
    mode(4);
    require(std::strcmp(tip.armed().header, initial.header) == 0 && preparations() == 1,
            "temporary read failure changed capture identity");
    mode(0); tip.disarmed();
    mode(1);
    require(std::strcmp(tip.armed().header, initial.header) != 0, "re-enable reused withdrawn identity");
    const auto reopened = tip.armed();
    mode(3);
    until([&] { return std::strcmp(tip.armed().header, reopened.header) != 0; }, "missed epoch change reused identity");

    // 静默改变共享边界，再从目录外移入文件，只有原子替换本身能唤醒观察者。
    const auto replacement = temporary / "replacement";
    { std::ofstream replacement_stream(replacement); replacement_stream << "synthetic-consent"; }
    mode(5);
    require(MoveFileExW(replacement.c_str(), (preferences / "consent-marker").c_str(),
                        MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != FALSE,
            "synthetic atomic replacement failed");
    tip.disarmed();
    mode(1); tip.armed();

    // 目录移走后旧观察句柄不能继续代表当前路径；恢复后仍须接到新目录的撤回。
    const auto moved = temporary / "preferences-moved";
    std::filesystem::rename(preferences, moved);
    tip.disarmed();
    require(std::filesystem::create_directory(preferences), "replacement preferences creation failed");
    mode(0);
    tip.disarmed();
    mode(1); tip.armed();
    mode(0); tip.disarmed();
    mode(1); tip.armed();
    std::filesystem::remove_all(preferences);
    tip.disarmed();
    require(std::filesystem::create_directory(preferences), "deleted preferences recreation failed");
    mode(1); tip.armed();

    // 等待生产读取确实被阻塞，再释放最后租约；停用必须在解除阻塞之前返回。
    block(1);
    require(entered(6000), "consent barrier not reached");
    const auto started = GetTickCount64();
    tip.release(second);
    require(GetTickCount64() - started < 250, "deactivation waited for blocked consent");
    tip.disarmed();
    tip.unload();
    require(GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") != nullptr, "pending observer lost loader reference");
    resume();
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "TIP stayed loaded after unregister");
  }
  {
    // 旧 begin 在后台返回时已经释放并重新激活，必须按新租约重新准备快照。
    Tip tip(directory);
    block(2);
    auto *old = tip.acquire();
    require(entered(6000), "prepare barrier not reached");
    const auto prepared = preparations();
    tip.release(old);
    auto *replacement = tip.acquire();
    Context before{};
    require(tip.snapshot(&before) && before.armed == 0, "replacement armed before fresh preparation");
    // 在生产刷新结束和下一轮观察之间固定时序，直接检查迟到的旧结果没有短暂武装。
    tip.block_refresh();
    resume();
    require(tip.wait_refresh(6000), "late prepare checkpoint not reached");
    Context late{};
    require(tip.snapshot(&late) && late.armed == 0, "late prepare armed replacement lease");
    tip.resume_refresh();
    tip.armed();
    require(preparations() == prepared + 1, "late prepare armed replacement lease");

    // 首次准备和暂停恢复都必须最终复核；稳定观察不再发布，故先明确制造这两种发布条件。
    for (const bool fresh_epoch : {false, true}) {
      const auto before_race = tip.armed();
      if (!fresh_epoch) {
        mode(2);
        tip.disarmed();
      }
      tip.block_publish();
      auto *race = tip.acquire();
      mode(fresh_epoch ? 3 : 1);
      require(tip.wait_publish(6000), "publish authorization barrier not reached");
      tip.block_refresh();
      mode(3);
      tip.resume_publish();
      require(tip.wait_refresh(6000), "revoked publish checkpoint not reached");
      Context revoked{};
      require(tip.snapshot(&revoked) && revoked.armed == 0,
              "revoked capture armed before authorization recheck");
      tip.resume_refresh();
      const auto after_race = tip.armed();
      require(std::strcmp(after_race.header, before_race.header) != 0,
              "revoked capture reused its old identity");
      tip.release(race);
    }

    // 系统注销失败时仍保留上下文与 DLL，重试成功后才能卸载。
    tip.fail_unregister(1);
    tip.release(replacement);
    until([&] { return tip.unregistered() >= 1; }, "unregister failure not exercised");
    Context waiting{};
    require(!tip.snapshot(&waiting) || !waiting.armed, "failed unregister rearmed context");
    tip.unload();
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "failed unregister did not retry safely");
  }
  {
    // 同一进程已有进程内记录器（水杉自己的设置与面板）时助手照常注册武装，快照标明记录器在，助手只补记绕过过滤器的快速失败。
    require(SetEnvironmentVariableW(msime::crash::in_process_recorder_variable,
                                    std::to_wstring(GetCurrentProcessId()).c_str()) != FALSE, "in-process marker failed");
    Tip tip(directory);
    auto *lease = tip.acquire();
    const auto beside = tip.armed();
    require(beside.in_process_recorder == 1 && tip.registered() == 1,
            "TIP did not mark WER capture beside the in-process recorder");
    // 记录器卸下或在 TIP 之后才装时，下一轮按同一身份重新发布标记。
    const auto prepared = preparations();
    const auto flagged = [&](std::uint32_t expected) {
      Context value{};
      return tip.snapshot(&value) && value.armed == 1 && value.in_process_recorder == expected &&
          std::strcmp(value.header, beside.header) == 0;
    };
    require(SetEnvironmentVariableW(msime::crash::in_process_recorder_variable, nullptr) != FALSE, "marker removal failed");
    mode(1);
    until([&] { return flagged(0); }, "removed in-process recorder kept its mark");
    require(SetEnvironmentVariableW(msime::crash::in_process_recorder_variable,
                                    std::to_wstring(GetCurrentProcessId()).c_str()) != FALSE, "late marker failed");
    mode(1);
    until([&] { return flagged(1); }, "late in-process recorder was not marked");
    require(preparations() == prepared && tip.registered() == 1, "recorder mark change re-prepared capture");
    tip.release(lease);
    tip.unload();
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "in-process mark leaked observer");
  }
  {
    // 从父进程继承到的标记带的是父进程号，外部宿主照常采集且不标明记录器。
    require(SetEnvironmentVariableW(msime::crash::in_process_recorder_variable,
                                    std::to_wstring(GetCurrentProcessId() + 1).c_str()) != FALSE, "inherited marker failed");
    Tip tip(directory);
    auto *lease = tip.acquire();
    require(tip.armed().in_process_recorder == 0 && tip.registered() == 1, "inherited marker changed TSF capture");
    tip.release(lease);
    tip.unload();
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "inherited marker leaked observer");
    require(SetEnvironmentVariableW(msime::crash::in_process_recorder_variable, nullptr) != FALSE, "marker cleanup failed");
  }
  for (unsigned index = 0; index < 20; ++index) {
    Tip tip(directory);
    auto *lease = tip.acquire();
    tip.armed();
    tip.release(lease);
    tip.unload();
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "repeated unload leaked observer");
  }
  require(invalid() == 0, "capture used invalid shared ABI request or barrier timed out");
  FreeLibrary(helper); finish(); FreeLibrary(host);
  std::puts("PASS TSF directory notifications, idle reads, shared leases, authorization and repeated DLL unload");
}
}
int main() {
  try { tests(); return 0; }
  catch (const std::exception &error) { std::fprintf(stderr, "FAIL: %s\n", error.what()); return 1; }
}
