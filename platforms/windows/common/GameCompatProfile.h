#pragma once
#include <windows.h>
#include <shellapi.h>
#include <cwchar>
#include <optional>
#include <string>
#include <vector>
#include "../../../shared/contracts/msime_edition.h"

// 移植自 msime-windows #925（e46294308df5a93d5f628878543a6a373cde3ccc），GPL-3.0，见根目录 LICENSE。
namespace msime::windows::game_profile {
inline constexpr wchar_t CompatName[] = L"拼音输入法";
inline constexpr wchar_t RecordValue[] = L"GameCompatProfileName";
inline constexpr wchar_t EnableSwitch[] = L"--enable-game-profile";
inline constexpr wchar_t DisableSwitch[] = L"--disable-game-profile";
inline std::wstring profile_key() {
    wchar_t language[16]{};
    swprintf(language, 16, L"0x%08x", MSIME_EDITION_LANGID);
    return std::wstring(L"SOFTWARE\\Microsoft\\CTF\\TIP\\") + MSIME_EDITION_CLSID_STRING +
           L"\\LanguageProfile\\" + language + L"\\" + MSIME_EDITION_PROFILE_GUID_STRING;
}
inline std::optional<std::wstring> read(const wchar_t *path, const wchar_t *name, REGSAM view) {
    HKEY key{};
    if (RegOpenKeyExW(HKEY_LOCAL_MACHINE, path, 0, KEY_QUERY_VALUE | view, &key) != ERROR_SUCCESS)
        return std::nullopt;
    wchar_t buffer[256]{};
    DWORD bytes = sizeof(buffer), type = 0;
    const auto status = RegQueryValueExW(key, name, nullptr, &type, reinterpret_cast<BYTE *>(buffer), &bytes);
    RegCloseKey(key);
    if (status != ERROR_SUCCESS || type != REG_SZ || bytes < sizeof(wchar_t) ||
        bytes > sizeof(buffer) || bytes % sizeof(wchar_t) || buffer[bytes / sizeof(wchar_t) - 1] != 0 ||
        std::wcslen(buffer) + 1 != bytes / sizeof(wchar_t))
        return std::nullopt;
    return std::wstring(buffer);
}
inline std::wstring registration_name() {
    const auto name = read(MSIME_EDITION_REGISTRY_KEY, RecordValue, KEY_WOW64_64KEY);
    return name && *name == CompatName ? *name : MSIME_EDITION_TEXT_SERVICE_DESCRIPTION;
}
inline bool enabled() { return registration_name() == CompatName; }
inline bool set(HKEY key, const wchar_t *name, const std::optional<std::wstring> &value) {
    if (!value) {
        const auto status = RegDeleteValueW(key, name);
        return status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND;
    }
    return RegSetValueExW(key, name, 0, REG_SZ, reinterpret_cast<const BYTE *>(value->c_str()),
                         static_cast<DWORD>((value->size() + 1) * sizeof(wchar_t))) == ERROR_SUCCESS;
}
// 先打开所有目标并保存原值，再修改；记录最后写入，失败恢复已写的 Description。
inline int apply_elevated(bool enable) {
    const auto path = profile_key();
    std::vector<HKEY> profiles;
    std::vector<std::optional<std::wstring>> previous;
    for (const auto view : {KEY_WOW64_64KEY, KEY_WOW64_32KEY}) {
        HKEY key{};
        const auto status = RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.c_str(), 0, KEY_QUERY_VALUE | KEY_SET_VALUE | view, &key);
        if (status == ERROR_FILE_NOT_FOUND || status == ERROR_PATH_NOT_FOUND) continue;
        if (status != ERROR_SUCCESS) {
            for (auto opened : profiles) RegCloseKey(opened);
            return 3;
        }
        profiles.push_back(key);
        previous.push_back(read(path.c_str(), L"Description", view));
        if (!previous.back()) {
            for (auto opened : profiles) RegCloseKey(opened);
            return 5;
        }
    }
    if (profiles.empty()) return 4;
    HKEY record{};
    if (RegCreateKeyExW(HKEY_LOCAL_MACHINE, MSIME_EDITION_REGISTRY_KEY, 0, nullptr, 0,
                        KEY_QUERY_VALUE | KEY_SET_VALUE | KEY_WOW64_64KEY, nullptr, &record, nullptr) != ERROR_SUCCESS) {
        for (auto key : profiles) RegCloseKey(key);
        return 3;
    }
    const std::optional<std::wstring> target{enable ? CompatName : MSIME_EDITION_TEXT_SERVICE_DESCRIPTION};
    bool ok = true;
    for (auto key : profiles) if (!set(key, L"Description", target)) { ok = false; break; }
    if (ok) ok = set(record, RecordValue, enable ? std::optional<std::wstring>{CompatName} : std::nullopt);
    bool restored = true;
    if (!ok) for (size_t n = 0; n < profiles.size(); ++n)
        if (!set(profiles[n], L"Description", previous[n])) restored = false;
    for (auto key : profiles) RegCloseKey(key);
    RegCloseKey(record);
    return ok ? 0 : (restored ? 5 : 6);
}
inline int run_if_requested(const wchar_t *command) {
    if (!command || !*command) return -1;
    int count = 0;
    auto args = CommandLineToArgvW(command, &count);
    if (!args) return -1;
    int result = -1;
    if (count && (wcscmp(args[0], EnableSwitch) == 0 || wcscmp(args[0], DisableSwitch) == 0))
        result = count == 1 ? apply_elevated(wcscmp(args[0], EnableSwitch) == 0) : 2;
    LocalFree(args);
    return result;
}
inline std::wstring apply(HWND owner, bool enable) {
    std::vector<wchar_t> path(32768);
    const auto length = GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
    if (!length || length >= path.size()) return L"找不到设置程序自身的路径。";
    SHELLEXECUTEINFOW execute{sizeof(execute)};
    execute.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    execute.hwnd = owner; execute.lpVerb = L"runas"; execute.lpFile = path.data();
    execute.lpParameters = enable ? EnableSwitch : DisableSwitch; execute.nShow = SW_HIDE;
    if (!ShellExecuteExW(&execute)) return GetLastError() == ERROR_CANCELLED ? L"已取消，登记名称没有更改。" : L"无法启动管理员操作。";
    if (!execute.hProcess) return L"无法取得管理员操作的结果，请重新打开设置核对登记名称。";
    const auto wait = WaitForSingleObject(execute.hProcess, 60000);
    DWORD code = 1;
    if (wait == WAIT_OBJECT_0) GetExitCodeProcess(execute.hProcess, &code);
    CloseHandle(execute.hProcess);
    if (wait != WAIT_OBJECT_0) return L"操作尚未结束，请重新打开设置核对登记名称。";
    if (code == 4) return L"输入法尚未在系统中注册，请重新安装。";
    if (code == 6) return L"修改失败且恢复不完整，请重新安装以修复登记名称。";
    if (code) return L"无法修改登记名称，已恢复原来的配置。";
    if (enabled() != enable) return L"登记记录与操作结果不一致，请重新打开设置核对。";
    return {};
}
} // namespace msime::windows::game_profile
