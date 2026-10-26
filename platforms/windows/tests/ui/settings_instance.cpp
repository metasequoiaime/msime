#include "../../settings/SettingsInstance.h"
#include <iostream>
#include <thread>

using Instance = msime::settings::SettingsInstance;

void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

struct Child {
  HANDLE handle = nullptr;
  explicit Child(HANDLE process) : handle(process) {}
  Child(const Child &) = delete;
  Child &operator=(const Child &) = delete;
  Child(Child &&other) noexcept : handle(std::exchange(other.handle, nullptr)) {}
  ~Child() {
    if (!handle) return;
    // 断言失败时也只回收本测试创建的进程，不能留下持锁的无限等待子进程。
    if (WaitForSingleObject(handle, 0) == WAIT_TIMEOUT) {
      TerminateProcess(handle, 99);
      WaitForSingleObject(handle, 5000);
    }
    CloseHandle(handle);
  }
};

Child child(const std::wstring &suffix, const std::string &route,
                          const wchar_t *mode = L"--child") {
  wchar_t executable[32768]{};
  require(GetModuleFileNameW(nullptr, executable, 32768) != 0, "executable unavailable");
  std::wstring command = L"\"" + std::wstring(executable) + L"\" " + mode + L" " + suffix +
                         L" " + std::wstring(route.begin(), route.end());
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  require(CreateProcessW(executable, command.data(), nullptr, nullptr, FALSE, 0,
                         nullptr, nullptr, &startup, &process) != FALSE, "child failed");
  CloseHandle(process.hThread);
  return Child(process.hProcess);
}

void finish(Child &process, DWORD expected, bool pump = true) {
  const auto deadline = GetTickCount64() + 15000;
  while (WaitForSingleObject(process.handle, 10) == WAIT_TIMEOUT) {
    if (pump) {
      MSG message{};
      while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
        TranslateMessage(&message);
        DispatchMessageW(&message);
      }
    }
    require(GetTickCount64() < deadline, "child timed out");
  }
  DWORD code = 0;
  require(GetExitCodeProcess(process.handle, &code) != FALSE && code == expected,
          "child activation outcome differs");
}

int main(int argc, char **argv) {
  try {
    if (argc == 4) {
      const std::string suffix(argv[2]);
      Instance instance(std::wstring(suffix.begin(), suffix.end()));
      const auto result = instance.start(argv[3], std::string_view(argv[1]) == "--timeout" ? 250 : 5000);
      if (std::string_view(argv[1]) == "--owner") {
        require(result == Instance::Start::primary, "owner not primary");
        const std::wstring ready = L"Local\\MSIME.Settings.Test.Ready." +
                                   std::wstring(suffix.begin(), suffix.end());
        const HANDLE event = OpenEventW(EVENT_MODIFY_STATE, FALSE, ready.c_str());
        require(event != nullptr && SetEvent(event), "owner ready failed");
        CloseHandle(event);
        Sleep(INFINITE);
      }
      return result == Instance::Start::forwarded ? 0 :
             result == Instance::Start::primary ? 10 : 20;
    }
    const auto suffix = L".synthetic-" + std::to_wstring(GetCurrentProcessId());
    WNDCLASSW descriptor{};
    descriptor.lpfnWndProc = DefWindowProcW;
    descriptor.hInstance = GetModuleHandleW(nullptr);
    descriptor.lpszClassName = L"MSIME.Settings.Test.Window";
    require(RegisterClassW(&descriptor) != 0, "window class unavailable");
    HWND window = CreateWindowExW(0, descriptor.lpszClassName, L"synthetic settings", WS_OVERLAPPEDWINDOW,
                                   0, 0, 320, 240, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    require(window != nullptr, "window unavailable");
    {
      Instance instance(suffix);
      require(instance.start("settings") == Instance::Start::primary, "first launch rejected");
      std::vector<Child> children;
      for (int index = 0; index < 8; ++index)
        children.push_back(child(suffix, index % 2 ? "settings:about" : "settings"));
      // 第二批启动先于主窗口发布入口，仍必须交给同一实例。
      Sleep(150);
      int activations = 0;
      int about = 0;
      require(instance.listen(window, [&](std::string_view route) {
        ++activations;
        if (route == "settings:about") ++about;
        return true;
      }), "listener unavailable");
      ShowWindow(window, SW_MINIMIZE);
      for (auto &process : children) finish(process, 0);
      require(activations == 8 && about == 4, "route lost or duplicate window permitted");
      if (!IsWindowVisible(window) || IsIconic(window)) {
        WINDOWPLACEMENT placement{};
        placement.length = sizeof(placement);
        GetWindowPlacement(window, &placement);
        std::cerr << "visible=" << IsWindowVisible(window) << " iconic=" << IsIconic(window)
                  << " show=" << placement.showCmd << '\n';
      }
      require(IsWindowVisible(window) && !IsIconic(window), "existing window not restored");
      auto invalid = child(suffix, "voice");
      finish(invalid, 20);
      auto malformed = child(suffix, "settings:about:extra");
      finish(malformed, 20);
      Instance other(suffix + L"-other-edition");
      require(other.start("settings") == Instance::Start::primary, "editions share guard");
    }
    {
      Instance restarted(suffix);
      require(restarted.start("settings") == Instance::Start::primary, "close prevents reopening");
      auto waiting = child(suffix, "settings", L"--timeout");
      finish(waiting, 20, false);
    }
    {
      const auto ready_name = L"Local\\MSIME.Settings.Test.Ready." + suffix;
      const HANDLE ready = CreateEventW(nullptr, TRUE, FALSE, ready_name.c_str());
      require(ready != nullptr, "ready unavailable");
      auto owner = child(suffix, "settings", L"--owner");
      require(WaitForSingleObject(ready, 5000) == WAIT_OBJECT_0, "owner not ready");
      // 等待中的新实例持有同一内核对象；旧进程异常退出后必须能接管废弃锁。
      std::thread crash([&] { Sleep(100); TerminateProcess(owner.handle, 99); });
      Instance recovered(suffix);
      const auto result = recovered.start("settings", 2000);
      crash.join();
      finish(owner, 99);
      CloseHandle(ready);
      require(result == Instance::Start::primary, "abandoned owner blocks restart");
    }
    DestroyWindow(window);
    std::cout << "settings instance: concurrent launch, routes, restore, timeout, isolation and recovery passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
