#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <delayimp.h>
#include <mmdeviceapi.h>
#include <oleauto.h>
#include <wrl.h>

#include <cstring>
#include <cwchar>

namespace {

// 延迟加载器默认搜索当前目录；兼容包缺少的系统 DLL 只能从系统目录探测。
HMODULE load_system_module(const wchar_t* name) noexcept {
    wchar_t path[MAX_PATH];
    const UINT length = GetSystemDirectoryW(path, MAX_PATH);
    const size_t name_length = std::wcslen(name);
    if (length == 0 || length >= MAX_PATH ||
        name_length + static_cast<size_t>(length) + 2 > MAX_PATH) {
        return nullptr;
    }
    path[length] = L'\\';
    std::memcpy(path + length + 1, name, (name_length + 1) * sizeof(wchar_t));
    return LoadLibraryW(path);
}

bool same_library(const char* value, const char* expected) noexcept {
    return value != nullptr && _stricmp(value, expected) == 0;
}

// 传统 COM 的 IErrorInfo 保留错误描述；HRESULT 仍由 windows-result 自己保存。
BOOL WINAPI originate_error(HRESULT, UINT length, PCWSTR message) noexcept {
    if (message == nullptr && length != 0) return FALSE;
    BSTR description = SysAllocStringLen(message, length);
    if (description == nullptr) return FALSE;
    ICreateErrorInfo* create = nullptr;
    HRESULT result = CreateErrorInfo(&create);
    if (SUCCEEDED(result)) {
        result = create->SetDescription(description);
        if (SUCCEEDED(result)) {
            IErrorInfo* error = nullptr;
            result = create->QueryInterface(__uuidof(IErrorInfo), reinterpret_cast<void**>(&error));
            if (SUCCEEDED(result)) {
                result = SetErrorInfo(0, error);
                error->Release();
            }
        }
        create->Release();
    }
    SysFreeString(description);
    return SUCCEEDED(result);
}

using Microsoft::WRL::ComPtr;

// cpal 的默认设备走虚拟 GUID；Win7 用普通 WASAPI 端点完成同一激活请求。
class AudioActivation final : public Microsoft::WRL::RuntimeClass<
    Microsoft::WRL::RuntimeClassFlags<Microsoft::WRL::ClassicCom>,
    IActivateAudioInterfaceAsyncOperation, Microsoft::WRL::FtmBase> {
public:
    void initialize(EDataFlow flow, REFIID iid, IActivateAudioInterfaceCompletionHandler* handler) noexcept {
        flow_ = flow;
        iid_ = iid;
        handler_ = handler;
    }

    HRESULT STDMETHODCALLTYPE GetActivateResult(HRESULT* result, IUnknown** activated) override {
        if (result == nullptr || activated == nullptr) return E_POINTER;
        *activated = nullptr;
        AcquireSRWLockShared(&lock_);
        *result = result_;
        const bool complete = complete_;
        if (complete && SUCCEEDED(result_)) activated_.CopyTo(activated);
        ReleaseSRWLockShared(&lock_);
        return complete ? S_OK : E_ILLEGAL_METHOD_CALL;
    }

    static DWORD WINAPI work(void* context) noexcept {
        auto* self = static_cast<AudioActivation*>(context);
        const HRESULT initialized = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
        self->complete(initialized);
        self->Release();
        if (SUCCEEDED(initialized)) CoUninitialize();
        return 0;
    }

private:
    void complete(HRESULT initialized) noexcept {
        ComPtr<IUnknown> activated;
        HRESULT result = initialized;
        if (SUCCEEDED(result)) {
            ComPtr<IMMDeviceEnumerator> enumerator;
            result = CoCreateInstance(__uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
                IID_PPV_ARGS(enumerator.GetAddressOf()));
            ComPtr<IMMDevice> device;
            if (SUCCEEDED(result)) result = enumerator->GetDefaultAudioEndpoint(flow_, eConsole, &device);
            if (SUCCEEDED(result)) {
                result = device->Activate(iid_, CLSCTX_ALL, nullptr,
                    reinterpret_cast<void**>(activated.GetAddressOf()));
            }
        }
        AcquireSRWLockExclusive(&lock_);
        result_ = result;
        activated_ = activated;
        complete_ = true;
        ReleaseSRWLockExclusive(&lock_);
        handler_->ActivateCompleted(this);
        handler_.Reset();
    }

    SRWLOCK lock_ = SRWLOCK_INIT;
    EDataFlow flow_ = eCapture;
    IID iid_{};
    ComPtr<IActivateAudioInterfaceCompletionHandler> handler_;
    ComPtr<IUnknown> activated_;
    HRESULT result_ = E_PENDING;
    bool complete_ = false;
};

HRESULT WINAPI activate_audio(
    LPCWSTR path, REFIID iid, PROPVARIANT* parameters, IActivateAudioInterfaceCompletionHandler* handler,
    IActivateAudioInterfaceAsyncOperation** operation) noexcept {
    if (operation == nullptr) return E_POINTER;
    *operation = nullptr;
    if (path == nullptr || handler == nullptr) return E_POINTER;
    // 进程回环及其参数属于新系统能力，产品的默认录音和播放不使用它们。
    if (parameters != nullptr) return E_NOTIMPL;
    EDataFlow flow;
    if (_wcsicmp(path, L"{2EEF81BE-33FA-4800-9670-1CD474972C3F}") == 0) {
        flow = eCapture;
    } else if (_wcsicmp(path, L"{E6327CAD-DCEC-4949-AE8A-991E976A79D2}") == 0) {
        flow = eRender;
    } else {
        return E_NOTIMPL;
    }
    auto pending = Microsoft::WRL::Make<AudioActivation>();
    if (!pending) return E_OUTOFMEMORY;
    pending->initialize(flow, iid, handler);
    pending->AddRef();
    if (!QueueUserWorkItem(&AudioActivation::work, pending.Get(), WT_EXECUTEDEFAULT)) {
        const DWORD error = GetLastError();
        pending->Release();
        return HRESULT_FROM_WIN32(error);
    }
    return pending.CopyTo(operation);
}

}  // namespace

// 只处理经导入审计确认的系统接口。未知 DLL 或函数仍由系统报告装载错误。
extern "C" FARPROC WINAPI msime_windows7_delay_notify(unsigned event, PDelayLoadInfo info) noexcept {
    if (info == nullptr) return nullptr;
    const bool combase = same_library(info->szDll, "combase.dll");
    const bool error_api = same_library(info->szDll, "api-ms-win-core-winrt-error-l1-1-0.dll");
    const bool audio = same_library(info->szDll, "mmdevapi.dll");
    if (!combase && !error_api && !audio) return nullptr;

    if (event == dliNotePreLoadLibrary) {
        const wchar_t* name = combase ? L"combase.dll" :
            error_api ? L"api-ms-win-core-winrt-error-l1-1-0.dll" : L"mmdevapi.dll";
        HMODULE module = load_system_module(name);
        if (module == nullptr && error_api) module = load_system_module(L"combase.dll");
        // Win7 没有 combase；旧 COM 函数原本就由 ole32 导出。
        // ole32 也作为缺失 API 的模块锚点，随后按函数名选择明确的失败实现。
        if (module == nullptr) module = load_system_module(L"ole32.dll");
        if (module == nullptr) {
            // 系统 COM 库都无法加载时终止装载，不能回到当前目录继续搜索。
            RaiseException(VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND), 0, 0, nullptr);
        }
        return reinterpret_cast<FARPROC>(module);
    }
    if (event != dliNotePreGetProcAddress || !info->dlp.fImportByName) return nullptr;
    const char* name = info->dlp.szProcName;
    if (name == nullptr) return nullptr;
    if (const FARPROC native = GetProcAddress(info->hmodCur, name)) return native;
    if (error_api && std::strcmp(name, "RoOriginateErrorW") == 0) {
        return reinterpret_cast<FARPROC>(&originate_error);
    }
    if (audio && std::strcmp(name, "ActivateAudioInterfaceAsync") == 0) {
        return reinterpret_cast<FARPROC>(&activate_audio);
    }
    return nullptr;
}

extern "C" const PfnDliHook __pfnDliNotifyHook2 = msime_windows7_delay_notify;
