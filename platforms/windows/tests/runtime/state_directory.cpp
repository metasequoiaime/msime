#include "StateDirectory.h"
#include <iostream>
#include <cwchar>
#include <stdexcept>

// The environment override is the one input a test can set without touching the machine: HKLM DataDir and the known folder are whatever this host has, so those cases only compare against the resolution without the override.
namespace {
void require(bool value, const char *message) {
  if (!value)
    throw std::runtime_error(message);
}
void set_override(const wchar_t *value) {
  require(SetEnvironmentVariableW(msime::windows::state_directory_environment_variable, value) != 0,
          "SetEnvironmentVariableW failed");
}

// 在可重定向的用户 Classes 子树内创建合成数据，不修改产品安装登记。
void registry_view_test() {
  struct Fixture {
    std::wstring path = L"Software\\Classes\\CLSID\\MSIME.RegistryView." +
        std::to_wstring(GetCurrentProcessId()) + L"." + std::to_wstring(GetTickCount64());
    ~Fixture() {
      RegDeleteKeyExW(HKEY_CURRENT_USER, path.c_str(), KEY_WOW64_64KEY, 0);
      RegDeleteKeyExW(HKEY_CURRENT_USER, path.c_str(), KEY_WOW64_32KEY, 0);
    }
    void write(REGSAM view, const wchar_t* text) {
      HKEY key = nullptr;
      require(RegCreateKeyExW(HKEY_CURRENT_USER, path.c_str(), 0, nullptr, 0,
                             KEY_SET_VALUE | view, nullptr, &key, nullptr) == ERROR_SUCCESS,
              "create synthetic registry key failed");
      const auto status = RegSetValueExW(key, L"DataDir", 0, REG_SZ,
          reinterpret_cast<const BYTE*>(text),
          static_cast<DWORD>((std::wcslen(text) + 1) * sizeof(wchar_t)));
      const DWORD number = 7;
      const auto type_status = RegSetValueExW(key, L"Number", 0, REG_DWORD,
          reinterpret_cast<const BYTE*>(&number), sizeof(number));
      // 多写一个终止符的 REG_SZ：读出来不能把它带进路径。
      const wchar_t padded[] = L"C:\\padded\0";
      const auto padded_status = RegSetValueExW(key, L"Padded", 0, REG_SZ,
          reinterpret_cast<const BYTE*>(padded), static_cast<DWORD>(sizeof(padded)));
      RegCloseKey(key);
      require(status == ERROR_SUCCESS && type_status == ERROR_SUCCESS && padded_status == ERROR_SUCCESS,
              "write synthetic registry values failed");
    }
  } fixture;
  const std::wstring expected = L"C:\\synthetic state\\安装目录";
  fixture.write(KEY_WOW64_64KEY, expected.c_str());
  BOOL wow64 = FALSE;
  require(IsWow64Process(GetCurrentProcess(), &wow64) != 0, "query WOW64 failed");
  if constexpr (sizeof(void*) == 8)
    wow64 = TRUE;
  if (wow64)
    fixture.write(KEY_WOW64_32KEY, L"C:\\wrong view");
  using msime::windows::read_registry_string_64;
  require(read_registry_string_64(HKEY_CURRENT_USER, fixture.path.c_str(), L"DataDir") == expected,
          "must read the 64-bit registry view, including on Win7 WOW64");
  require(read_registry_string_64(HKEY_CURRENT_USER, fixture.path.c_str(), L"Padded") == L"C:\\padded",
          "extra terminators must not reach the path");
  require(read_registry_string_64(HKEY_CURRENT_USER, fixture.path.c_str(), L"Missing").empty(),
          "missing registry values must remain empty");
  require(read_registry_string_64(HKEY_CURRENT_USER, fixture.path.c_str(), L"Number").empty(),
          "non-string registry values must be rejected");
  require(read_registry_string_64(HKEY_CURRENT_USER, (fixture.path + L"\\Missing").c_str(), L"DataDir").empty(),
          "missing registry keys must remain empty");
}
} // namespace

int main() {
  try {
    registry_view_test();
    set_override(nullptr);
    const auto without_override = msime::windows::resolve_state_directory();
    require(without_override.empty() || without_override.is_absolute(),
            "The resolved root must be absolute");

    // An absolute override wins over DataDir and LocalAppData. Set through the process environment block, the way a launcher injects it, so this also covers the lookup not going through the CRT's copy.
    const std::filesystem::path configured = L"C:\\msime state\\override";
    set_override(configured.c_str());
    require(msime::windows::resolve_state_directory() == configured,
            "An absolute override must win");

    // A relative or empty override is ignored rather than resolved against the working directory.
    set_override(L"relative\\state");
    require(msime::windows::resolve_state_directory() == without_override,
            "A relative override must be ignored");
    set_override(L"");
    require(msime::windows::resolve_state_directory() == without_override,
            "An empty override must be ignored");

    set_override(nullptr);
    std::cout << "state directory tests passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}
