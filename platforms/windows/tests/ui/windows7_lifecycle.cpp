#include "../../settings/win7/SettingsWindow.h"
#include <objbase.h>
#include <cstdio>
#include <filesystem>
#include <stdexcept>

namespace {
void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}
struct StateDirectory {
  std::filesystem::path path;
  std::wstring previous;
  bool present = false;
  StateDirectory() {
    wchar_t root[MAX_PATH]{}, unique[MAX_PATH]{};
    require(GetTempPathW(MAX_PATH, root) != 0, "Temporary root unavailable");
    require(GetTempFileNameW(root, L"m7l", 0, unique) != 0, "Temporary name unavailable");
    require(DeleteFileW(unique) && CreateDirectoryW(unique, nullptr), "Temporary directory unavailable");
    path = unique;
    const auto length = GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", nullptr, 0);
    present = length != 0;
    if (present) {
      previous.resize(length);
      previous.resize(GetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", previous.data(), length));
    }
    require(SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", path.c_str()) != FALSE,
            "Isolated state unavailable");
  }
  ~StateDirectory() {
    SetEnvironmentVariableW(L"MSIME_CLIENT_STATE_DIR", present ? previous.c_str() : nullptr);
    std::error_code error;
    std::filesystem::remove_all(path, error);
  }
};
void create_and_destroy() {
  msime::settings::win7::SettingsWindow window;
  require(window.create(GetModuleHandleW(nullptr)), "Settings window creation failed");
  // 设置窗口拥有自己的 COM 初始化；销毁时必须先释放帮助按钮的无障碍对象。
}
} // namespace
int main() {
  try {
    StateDirectory state;
    ULONG_PTR token = 0;
    require(CoGetContextToken(&token) == CO_E_NOTINITIALIZED, "Unexpected ambient COM apartment");
    create_and_destroy();
    require(CoGetContextToken(&token) == CO_E_NOTINITIALIZED, "Settings leaked a COM apartment");
    require(SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)), "Outer COM initialization failed");
    create_and_destroy();
    const auto preserved = CoGetContextToken(&token);
    CoUninitialize();
    require(SUCCEEDED(preserved), "Settings closed the caller's COM apartment");
    require(CoGetContextToken(&token) == CO_E_NOTINITIALIZED, "Outer COM apartment leaked");
    std::puts("Windows 7 lifecycle: owned and caller COM apartments survived window teardown");
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "Windows 7 lifecycle: %s\n", error.what());
    return 1;
  }
}
