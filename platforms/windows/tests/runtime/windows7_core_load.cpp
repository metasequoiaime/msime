#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <objbase.h>
#include "../../../../crates/host-api/include/msime_client.h"

#include <cassert>
#include <cstdio>
#include <cstring>
#include <initializer_list>

template<class Function>
Function entry(HMODULE module, const char* name) {
    const auto address = GetProcAddress(module, name);
    assert(address != nullptr);
    return reinterpret_cast<Function>(reinterpret_cast<void*>(address));
}

int wmain(int argc, wchar_t** argv) {
    assert(argc == 2);
    const HMODULE module = LoadLibraryExW(argv[1], nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (module == nullptr) {
        std::fprintf(stderr, "Core DLL load failed: %lu\n", GetLastError());
        return 1;
    }
    const auto version = entry<decltype(&msime_client_abi_version)>(module, "msime_client_abi_version");
    const auto release = entry<decltype(&msime_client_string_free)>(module, "msime_client_string_free");
    assert(version() == 3);
    const HRESULT initialized = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    assert(SUCCEEDED(initialized));
    // 触发实际 DirectWrite、COM 分配释放和音频枚举；不录音、不输出字体或设备身份。
    for (const char* name : {"msime_client_list_font_families", "msime_client_voice_capture_devices"}) {
        const auto enumerate = entry<char* (*)()>(module, name);
        char* response = enumerate();
        assert(response != nullptr && std::strstr(response, "\"ok\":true") != nullptr);
        release(response);
    }
    CoUninitialize();
    FreeLibrary(module);
    std::puts("Win7 core DLL: ABI, font catalog and audio enumeration passed");
}
