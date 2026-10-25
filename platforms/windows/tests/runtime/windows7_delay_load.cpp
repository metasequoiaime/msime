#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <delayimp.h>
#include <mmdeviceapi.h>
#include <audioclient.h>
#include <oleauto.h>
#include <wrl.h>

#include <cassert>
#include <cwchar>
#include <initializer_list>
#include <string>

extern "C" FARPROC WINAPI msime_windows7_delay_notify(unsigned, PDelayLoadInfo) noexcept;

namespace {
class Completion final : public Microsoft::WRL::RuntimeClass<
    Microsoft::WRL::RuntimeClassFlags<Microsoft::WRL::ClassicCom>,
    IActivateAudioInterfaceCompletionHandler, Microsoft::WRL::FtmBase> {
public:
    Completion() : event(CreateEventW(nullptr, TRUE, FALSE, nullptr)) { assert(event != nullptr); }
    ~Completion() { CloseHandle(event); }
    HRESULT STDMETHODCALLTYPE ActivateCompleted(IActivateAudioInterfaceAsyncOperation* operation) override {
        assert(operation != nullptr);
        assert(operation->GetActivateResult(&result, &activated) == S_OK);
        SetEvent(event);
        return S_OK;
    }
    HANDLE event;
    HRESULT result = E_PENDING;
    Microsoft::WRL::ComPtr<IUnknown> activated;
};

FARPROC probe(const char* library, const char* name, HMODULE module) {
    DelayLoadInfo info{};
    info.szDll = library;
    info.dlp.fImportByName = TRUE;
    info.dlp.szProcName = name;
    info.hmodCur = module;
    return msime_windows7_delay_notify(dliNotePreGetProcAddress, &info);
}

HMODULE load(const char* library) {
    DelayLoadInfo info{};
    info.szDll = library;
    return reinterpret_cast<HMODULE>(msime_windows7_delay_notify(dliNotePreLoadLibrary, &info));
}
}  // namespace

int main() {
    const HRESULT initialized = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    assert(SUCCEEDED(initialized));
    assert(msime_windows7_delay_notify(dliNotePreLoadLibrary, nullptr) == nullptr);
    assert(load("unrelated-synthetic-library.dll") == nullptr);
    HMODULE ole32 = GetModuleHandleW(L"ole32.dll");
    assert(ole32 != nullptr);

    // 原生接口按同一个模块的地址转发，不能在现代系统上强制使用回退。
    for (const char* library : {"combase.dll", "mmdevapi.dll", "api-ms-win-core-winrt-error-l1-1-0.dll"}) {
        HMODULE module = load(library);
        assert(module != nullptr);
        wchar_t path[MAX_PATH];
        const DWORD length = GetModuleFileNameW(module, path, MAX_PATH);
        assert(length != 0 && length < MAX_PATH);
        wchar_t system[MAX_PATH];
        const UINT system_length = GetSystemDirectoryW(system, MAX_PATH);
        assert(system_length != 0 && system_length < MAX_PATH);
        // Win7 WOW64 的已加载模块可能仍报告 System32 别名；用明确的系统路径重新加载并核对模块身份。
        const auto filename = std::wcsrchr(path, L'\\'); assert(filename);
        const auto expected = LoadLibraryW((std::wstring(system, system_length) + filename).c_str());
        assert(expected == module);
        FreeLibrary(expected);
        const char* name = library[0] == 'c' ? "CoTaskMemFree" :
            library[0] == 'm' ? "ActivateAudioInterfaceAsync" : "RoOriginateErrorW";
        if (const FARPROC native = GetProcAddress(module, name)) {
            assert(probe(library, name, module) == native);
        }
        FreeLibrary(module);
    }
    assert(probe("combase.dll", "CoTaskMemFree", ole32) == GetProcAddress(ole32, "CoTaskMemFree"));
    assert(probe("combase.dll", "CoCreateFreeThreadedMarshaler", ole32) ==
        GetProcAddress(ole32, "CoCreateFreeThreadedMarshaler"));
    assert(probe("combase.dll", "SyntheticUnknown", ole32) == nullptr);
    assert(probe("unrelated.dll", "CoTaskMemFree", ole32) == nullptr);

    // 用已知没有新音频入口的 ole32 模拟旧系统，核对错误码及输出所有权。
    using Activate = HRESULT (WINAPI*)(LPCWSTR, REFIID, PROPVARIANT*,
        IActivateAudioInterfaceCompletionHandler*, IActivateAudioInterfaceAsyncOperation**);
    const auto activate = reinterpret_cast<Activate>(probe("mmdevapi.dll", "ActivateAudioInterfaceAsync", ole32));
    assert(activate != nullptr);
    auto* operation = reinterpret_cast<IActivateAudioInterfaceAsyncOperation*>(1);
    const GUID iid{};
    assert(activate(nullptr, iid, nullptr, nullptr, &operation) == E_POINTER);
    assert(operation == nullptr);
    assert(activate(nullptr, iid, nullptr, nullptr, nullptr) == E_POINTER);
    for (const wchar_t* path : {L"{2EEF81BE-33FA-4800-9670-1CD474972C3F}",
                               L"{E6327CAD-DCEC-4949-AE8A-991E976A79D2}"}) {
        auto completed = Microsoft::WRL::Make<Completion>();
        Microsoft::WRL::ComPtr<IActivateAudioInterfaceAsyncOperation> pending;
        assert(activate(path, __uuidof(IAudioClient), nullptr, completed.Get(), &pending) == S_OK);
        assert(pending != nullptr);
        assert(WaitForSingleObject(completed->event, 10000) == WAIT_OBJECT_0);
        assert(completed->result != E_PENDING && completed->result != E_NOTIMPL);
        assert(SUCCEEDED(completed->result) == (completed->activated != nullptr));
        HRESULT again = E_PENDING;
        Microsoft::WRL::ComPtr<IUnknown> again_activated;
        assert(pending->GetActivateResult(&again, &again_activated) == S_OK);
        assert(again == completed->result);
        assert((again_activated != nullptr) == (completed->activated != nullptr));
    }
    auto completed = Microsoft::WRL::Make<Completion>();
    assert(activate(L"SyntheticUnsupported", iid, nullptr, completed.Get(), &operation) == E_NOTIMPL);
    assert(operation == nullptr);

    // 错误消息经真实 COM 线程错误槽往返；长度之外的合成文本不得泄漏进去。
    using Originate = BOOL (WINAPI*)(HRESULT, UINT, PCWSTR);
    const auto originate = reinterpret_cast<Originate>(
        probe("api-ms-win-core-winrt-error-l1-1-0.dll", "RoOriginateErrorW", ole32));
    assert(originate != nullptr);
    assert(originate(E_FAIL, 9, L"SyntheticIgnored") == TRUE);
    IErrorInfo* error = nullptr;
    assert(GetErrorInfo(0, &error) == S_OK && error != nullptr);
    BSTR description = nullptr;
    assert(error->GetDescription(&description) == S_OK);
    assert(SysStringLen(description) == 9 && std::wcscmp(description, L"Synthetic") == 0);
    SysFreeString(description);
    error->Release();
    assert(originate(E_FAIL, 1, nullptr) == FALSE);
    CoUninitialize();
}
