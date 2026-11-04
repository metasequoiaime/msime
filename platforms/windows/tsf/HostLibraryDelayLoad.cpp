#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "Global/Globals.h"
#include "ModulePath.h"
#include "../../../shared/contracts/msime_edition.h"

#include <windows.h>
#include <delayimp.h>

#include <cstring>
#include <string>

namespace {

bool same_library(const char *actual, const char *expected) noexcept {
    return actual != nullptr && expected != nullptr && _stricmp(actual, expected) == 0;
}

std::wstring tsf_directory() {
    const auto module = msime::tsf::ReadModulePath([](wchar_t *buffer, unsigned capacity) {
        return GetModuleFileNameW(Global::dllInstanceHandle, buffer, capacity);
    });
    if (module.empty()) return {};
    const auto separator = module.find_last_of(L"\\/");
    if (separator == std::wstring::npos) return {};
    return module.substr(0, separator);
}

HMODULE load_host(const wchar_t *name) noexcept {
    auto directory = tsf_directory();
    if (directory.empty()) return nullptr;
    directory.push_back(L'\\');
    directory.append(name);
    return LoadLibraryExW(directory.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
}

HMODULE fail_load() noexcept {
    RaiseException(VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND), 0, 0, nullptr);
    return nullptr;
}

}  // namespace

// 系统进程加载 TSF 时不保证当前目录或 PATH 包含安装目录；只接受版本表生成的两个宿主名字，并从 TSF 同目录加载，避免误载其他版本。
extern "C" FARPROC WINAPI msime_host_delay_notify(unsigned event, PDelayLoadInfo info) noexcept {
    if (event != dliNotePreLoadLibrary || info == nullptr) return nullptr;
    const bool host = same_library(info->szDll, MSIME_EDITION_HOST_DLL_A);
    const bool arm64_host = same_library(info->szDll, MSIME_EDITION_HOST_DLL_ARM64_A);
    if (!host && !arm64_host) return nullptr;
    const auto module = load_host(arm64_host ? MSIME_EDITION_HOST_DLL_ARM64 : MSIME_EDITION_HOST_DLL);
    return reinterpret_cast<FARPROC>(module != nullptr ? module : fail_load());
}

extern "C" const PfnDliHook __pfnDliNotifyHook2 = msime_host_delay_notify;
