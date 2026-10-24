#include <windows.h>
#include <msctf.h>
#include <imm.h>
#if __has_include(<immdev.h>)
#include <immdev.h>
#endif
#include <wrl/client.h>
#include "../../../../shared/contracts/ipc_negotiation.h"
#include <cstdio>
#include <functional>
#include <stdexcept>
#include <string>

namespace {
#ifndef IMC_SETOPENSTATUS
// MinGW 不提供 immdev.h；沿用 Windows SDK 固定的 WM_IME_CONTROL 命令值。
constexpr WPARAM IMC_SETOPENSTATUS = 0x0006;
#endif
#ifndef TF_CLIENTID_NULL
constexpr TfClientId TF_CLIENTID_NULL = 0;
#endif
#ifndef TF_IPPMF_FORSESSION
// 与 Windows SDK 的会话级激活标记保持一致，不退回进程范围。
constexpr DWORD TF_IPPMF_FORSESSION = 0x20000000;
#endif
using Microsoft::WRL::ComPtr;
void require(bool value, const char* reason) {
  if (!value) throw std::runtime_error(reason);
}
void pump() {
  MSG message{};
  while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
    TranslateMessage(&message); DispatchMessageW(&message);
  }
}
void wait_for(const std::function<bool()>& condition, const char* reason) {
  const auto deadline = GetTickCount64() + 20000;
  do {
    pump();
    if (condition()) return;
    MsgWaitForMultipleObjects(0, nullptr, FALSE, 20, QS_ALLINPUT);
  } while (GetTickCount64() < deadline);
  throw std::runtime_error(reason);
}
struct ProductPipe {
  HANDLE handle = INVALID_HANDLE_VALUE;
  ProductPipe(const wchar_t* name, bool asynchronous) {
    const DWORD flags = SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION |
                        (asynchronous ? FILE_FLAG_OVERLAPPED : 0);
    handle = CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, flags, nullptr);
    if (handle == INVALID_HANDLE_VALUE && GetLastError() == ERROR_PIPE_BUSY && WaitNamedPipeW(name, 2000))
      handle = CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, flags, nullptr);
    require(handle != INVALID_HANDLE_VALUE, "Cannot open an installed product pipe");
    ULONG session = 0; DWORD own = 0;
    require(GetNamedPipeServerSessionId(handle, &session) && ProcessIdToSessionId(GetCurrentProcessId(), &own) && session == own,
            "Installed product pipe belongs to a different session");
    DWORD mode = PIPE_READMODE_MESSAGE | (asynchronous ? PIPE_WAIT : PIPE_NOWAIT);
    require(SetNamedPipeHandleState(handle, &mode, nullptr, nullptr) != FALSE, "Cannot set installed product pipe mode");
  }
  void exchange(void* data, DWORD size, bool write) const {
    OVERLAPPED pending{}; pending.hEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
    require(pending.hEvent != nullptr, "Cannot create a protocol test event");
    DWORD transferred = 0;
    BOOL done = write ? WriteFile(handle, data, size, &transferred, &pending)
                      : ReadFile(handle, data, size, &transferred, &pending);
    const auto error = done ? ERROR_SUCCESS : GetLastError();
    if (!done && error == ERROR_IO_PENDING) {
      if (WaitForSingleObject(pending.hEvent, 2000) != WAIT_OBJECT_0) CancelIoEx(handle, &pending);
      done = GetOverlappedResult(handle, &pending, &transferred, TRUE);
    }
    CloseHandle(pending.hEvent);
    require(done && transferred == size, write ? "Installed pipe hello write failed" : "Installed pipe readiness acknowledgement failed");
  }
  ~ProductPipe() { if (handle != INVALID_HANDLE_VALUE) CloseHandle(handle); }
};
void verify_product_transport() {
  const uint64_t client = (static_cast<uint64_t>(GetCurrentProcessId()) << 32) | 1;
  ProductPipe replies(FANY_IME_TO_TSF_NAMED_PIPE, true);
  FanyImePipeHello reverse{}; reverse.client_id = client; reverse.pipe_role = FanyImePipeRole::ToTsf;
  replies.exchange(&reverse, sizeof(reverse), true);
  FanyImeNamedpipeDataToTsf ready{}; replies.exchange(&ready, sizeof(ready), false);
  require(ready.msg_type == FanyImeReplyType::PipeReady, "Installed reply pipe sent an invalid readiness acknowledgement");
  std::puts("Installed reply pipe acknowledged");
  ProductPipe worker(FANY_IME_TO_TSF_WORKER_THREAD_NAMED_PIPE, true);
  reverse.pipe_role = FanyImePipeRole::ToTsfWorkerThread;
  worker.exchange(&reverse, sizeof(reverse), true);
  FanyImeNamedpipeDataToTsfWorkerThread worker_ready{}; worker.exchange(&worker_ready, sizeof(worker_ready), false);
  require(worker_ready.msg_type == FanyImeWorkerReplyType::PipeReady, "Installed worker pipe sent an invalid readiness acknowledgement");
  std::puts("Installed worker pipe acknowledged");
  ProductPipe main(FANY_IME_NAMED_PIPE, false);
  const auto hello = FanyImeProtocol::Hello(client, 1);
  DWORD transferred = 0;
  require(WriteFile(main.handle, &hello, sizeof(hello), &transferred, nullptr) && transferred == sizeof(hello),
          "Installed main pipe negotiation write failed");
  replies.exchange(&ready, sizeof(ready), false);
  require(FanyImeProtocol::AcceptReply(ready, 1), "Installed product protocol negotiation failed");
  std::puts("Installed main pipe protocol negotiated");
}
struct ComSession {
  ComSession() { require(SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)), "COM initialization failed"); }
  ~ComSession() { CoUninitialize(); }
};
struct InputProfile {
  ComPtr<ITfInputProcessorProfileMgr> manager;
  ComPtr<ITfThreadMgr> thread_manager;
  TfClientId client_id = TF_CLIENTID_NULL;
  TF_INPUTPROCESSORPROFILE previous{};
  HKL original_layout = GetKeyboardLayout(0);
  bool saved = false;
  bool restored = false;
  InputProfile() {
    require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr, nullptr, CLSCTX_INPROC_SERVER,
                                      IID_PPV_ARGS(&thread_manager))) &&
            SUCCEEDED(thread_manager->Activate(&client_id)), "Cannot activate the probe TSF thread manager");
    require(SUCCEEDED(CoCreateInstance(CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
                                      IID_PPV_ARGS(&manager))), "TSF profile manager unavailable");
    saved = manager->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &previous) == S_OK;
    require(saved, "Cannot preserve the original desktop input profile");
  }
  void activate(HWND editor) {
    // 激活会话内的已注册 TIP，不修改用户的启用状态或默认输入法。
    const auto chinese = LoadKeyboardLayoutW(L"00000804", KLF_ACTIVATE);
    require(chinese != nullptr, "Chinese keyboard layout unavailable");
    // Win7 仅切换调用线程的布局；先让自己创建的编辑器切到相同语言，再激活该语言的 TIP。
    DWORD_PTR accepted = 0;
    require(SendMessageTimeoutW(editor, WM_INPUTLANGCHANGEREQUEST, 0, reinterpret_cast<LPARAM>(chinese),
                                SMTO_ABORTIFHUNG, 2000, &accepted) != 0, "Cannot switch the owned editor input language");
    wait_for([&] { return LOWORD(reinterpret_cast<ULONG_PTR>(GetKeyboardLayout(GetWindowThreadProcessId(editor, nullptr)))) == MSIME_EDITION_LANGID; },
             "Owned editor input language did not switch");
    constexpr CLSID service = MSIME_EDITION_CLSID;
    constexpr GUID profile = MSIME_EDITION_PROFILE_GUID;
    const auto result = manager->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR, MSIME_EDITION_LANGID,
                                                 service, profile, nullptr, TF_IPPMF_FORSESSION);
    require(result == S_OK, "Registered TSF profile activation failed");
    // Win7 的会话切换由消息分发完成，等待实际配置，不能把 ActivateProfile 返回成功当作已切换。
    wait_for([&] {
      TF_INPUTPROCESSORPROFILE active{};
      return manager->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD, &active) == S_OK &&
             IsEqualCLSID(active.clsid, service) && IsEqualGUID(active.guidProfile, profile);
    }, "The requested TSF profile did not become active");
  }
  bool restore() {
    if (restored) return true;
    bool success = true;
    if (saved) {
      ActivateKeyboardLayout(previous.hkl ? previous.hkl : original_layout, 0);
      success = manager->ActivateProfile(previous.dwProfileType, previous.langid, previous.clsid,
                                         previous.guidProfile, previous.hkl, TF_IPPMF_FORSESSION) == S_OK;
    }
    success = ActivateKeyboardLayout(original_layout, 0) != nullptr && success;
    restored = success;
    return success;
  }
  ~InputProfile() { restore(); if (client_id != TF_CLIENTID_NULL) thread_manager->Deactivate(); }
};
struct Editor {
  PROCESS_INFORMATION process{};
  HWND window = nullptr, edit = nullptr;
  Editor() {
    wchar_t system[MAX_PATH]{};
    require(GetSystemDirectoryW(system, MAX_PATH) != 0, "System directory unavailable");
    const auto executable = std::wstring(system) + L"\\notepad.exe";
    auto command = L"\"" + executable + L"\"";
    STARTUPINFOW startup{}; startup.cb = sizeof(startup);
    require(CreateProcessW(executable.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr,
                           nullptr, &startup, &process) != FALSE, "Cannot start a new synthetic Notepad session");
  }
  void ready() {
    BOOL own_wow64 = FALSE, editor_wow64 = FALSE;
    require(IsWow64Process(GetCurrentProcess(), &own_wow64) && IsWow64Process(process.hProcess, &editor_wow64) &&
            own_wow64 == editor_wow64, "The system Notepad architecture does not match this test process");
    wait_for([&] {
      EnumWindows([](HWND candidate, LPARAM data) -> BOOL {
        auto& self = *reinterpret_cast<Editor*>(data);
        DWORD pid = 0; GetWindowThreadProcessId(candidate, &pid);
        if (pid == self.process.dwProcessId && IsWindowVisible(candidate)) {
          const auto child = FindWindowExW(candidate, nullptr, L"Edit", nullptr);
          if (child) { self.window = candidate; self.edit = child; return FALSE; }
        }
        return TRUE;
      }, reinterpret_cast<LPARAM>(this));
      return edit != nullptr;
    }, "The owned Notepad Edit control was not created");
  }
  void foreground() {
    const auto foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), nullptr);
    const auto current_thread = GetCurrentThreadId();
    const bool attached = foreground_thread && foreground_thread != current_thread &&
                          AttachThreadInput(current_thread, foreground_thread, TRUE);
    ShowWindow(window, SW_RESTORE); SetForegroundWindow(window); BringWindowToTop(window);
    if (attached) AttachThreadInput(current_thread, foreground_thread, FALSE);
    wait_for([&] { return GetForegroundWindow() == window; }, "Cannot focus the owned Notepad session");
  }
  void chinese_mode() const {
    // 只打开本测试编辑器的临时输入状态，不修改产品的英文出厂默认值。
    const auto ime = ImmGetDefaultIMEWnd(edit);
    require(ime != nullptr, "The owned editor has no default IME window");
    DWORD_PTR ignored = 0;
    require(SendMessageTimeoutW(ime, WM_IME_CONTROL, IMC_SETOPENSTATUS, TRUE,
                                SMTO_ABORTIFHUNG, 2000, &ignored) != 0,
            "Cannot select Chinese mode in the owned editor");
    pump();
  }
  std::wstring text() const {
    wchar_t buffer[4096]{}; DWORD_PTR length = 0;
    require(SendMessageTimeoutW(edit, WM_GETTEXT, 4096, reinterpret_cast<LPARAM>(buffer),
                                SMTO_ABORTIFHUNG, 2000, &length) != 0 && length < 4096,
            "Cannot read synthetic text from the owned Notepad session");
    return {buffer, static_cast<std::size_t>(length)};
  }
  ~Editor() {
    // 只清空并关闭本测试创建的记事本，避免保存对话框；绝不枚举和清理用户文档。
    if (edit) SendMessageTimeoutW(edit, WM_SETTEXT, 0, reinterpret_cast<LPARAM>(L""), SMTO_ABORTIFHUNG, 2000, nullptr);
    if (window) PostMessageW(window, WM_CLOSE, 0, 0);
    if (process.hProcess) {
      if (WaitForSingleObject(process.hProcess, 5000) == WAIT_TIMEOUT) TerminateProcess(process.hProcess, 1);
      CloseHandle(process.hThread); CloseHandle(process.hProcess);
    }
  }
};
void press(HWND target, WORD key) {
  require(GetForegroundWindow() == target, "Foreground changed during synthetic keyboard input");
  // 使用真实虚拟键按下/抬起，禁止 KEYEVENTF_UNICODE 或直接写入编辑框冒充 TSF 输入。
  INPUT inputs[2]{};
  inputs[0].type = inputs[1].type = INPUT_KEYBOARD;
  inputs[0].ki.wVk = inputs[1].ki.wVk = key;
  inputs[1].ki.dwFlags = KEYEVENTF_KEYUP;
  require(SendInput(2, inputs, sizeof(INPUT)) == 2, "Synthetic keyboard input was rejected");
  Sleep(120); pump();
}
}
int main(int count, char** arguments) {
  if (count != 2 || std::string(arguments[1]) != "--desktop") {
    std::puts("Use --desktop in a disposable interactive Windows session with this edition installed.");
    return 2;
  }
  try {
    USEROBJECTFLAGS flags{}; DWORD bytes = 0;
    require(GetUserObjectInformationW(GetProcessWindowStation(), UOI_FLAGS, &flags, sizeof(flags), &bytes) &&
            (flags.dwFlags & WSF_VISIBLE), "A visible interactive desktop is required");
    verify_product_transport();
    ComSession com;
    InputProfile profile;
    Editor editor; editor.ready(); editor.foreground();
    require(editor.text().empty(), "The new Notepad document must be empty");
    profile.activate(editor.window);
    Sleep(1000); pump();
    editor.chinese_mode();
    std::printf("Owned editor layout=%08lx; probe layout=%08lx\n",
        static_cast<unsigned long>(reinterpret_cast<ULONG_PTR>(GetKeyboardLayout(GetWindowThreadProcessId(editor.window, nullptr)))),
        static_cast<unsigned long>(reinterpret_cast<ULONG_PTR>(GetKeyboardLayout(0))));
    HWND worker = nullptr;
    unsigned owned_workers = 0;
    while ((worker = FindWindowExW(HWND_MESSAGE, worker, L"MetasequoiaIMEWorkerWnd" MSIME_EDITION_NAME_SUFFIX, nullptr))) {
      DWORD pid = 0; GetWindowThreadProcessId(worker, &pid);
      if (pid == editor.process.dwProcessId) ++owned_workers;
    }
    std::printf("Owned editor TIP worker count=%u\n", owned_workers);
    for (const auto key : std::string("NIHAO")) press(editor.window, static_cast<WORD>(key));
    wait_for([] {
      const auto candidate = FindWindowW(L"MSIME.Client.Preview.Candidates" MSIME_EDITION_NAME_SUFFIX, nullptr);
      return candidate && IsWindowVisible(candidate);
    }, "TSF keystrokes did not display the installed Server candidate window");
    press(editor.window, VK_SPACE);
    wait_for([&] { return editor.text() == L"\u4f60\u597d"; }, "TSF candidate selection did not commit the expected synthetic Chinese text");
    require(profile.restore(), "Cannot restore the original desktop input profile");
    std::printf("PASS %u-bit system Notepad: registered TIP, real keys, visible candidates and Chinese commit\n",
                static_cast<unsigned>(sizeof(void*) * 8));
    return 0;
  } catch (const std::exception& error) {
    std::fprintf(stderr, "%s\n", error.what()); return 1;
  }
}
