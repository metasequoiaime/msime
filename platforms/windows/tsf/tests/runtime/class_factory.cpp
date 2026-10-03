#include <windows.h>
#include <objbase.h>
#include <msctf.h>

#include <stdexcept>
#include <string>
#include <cstring>

#include "../../../../../shared/contracts/msime_edition.h"

namespace {
// 本次构建的版本的 CLSID：DLL 只为自己版本的 CLSID 给出类工厂。
constexpr CLSID kMetasequoiaImeClsid = MSIME_EDITION_CLSID;
constexpr CLSID kUnknownClsid = {
    0x4c8a4f2b, 0x2c98, 0x4f85, {0x9e, 0x40, 0x62, 0x35, 0x8c, 0x1b, 0x91, 0x77}};

using DllGetClassObjectFn = HRESULT(STDAPICALLTYPE *)(REFCLSID, REFIID, void **);
using DllCanUnloadNowFn = HRESULT(STDAPICALLTYPE *)(void);

void require(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}

std::wstring module_directory() {
  wchar_t path[MAX_PATH]{};
  const DWORD length = GetModuleFileNameW(nullptr, path, ARRAYSIZE(path));
  require(length != 0 && length < ARRAYSIZE(path), "Could not locate test executable");
  std::wstring result(path, length);
  const auto slash = result.find_last_of(L"\\/");
  require(slash != std::wstring::npos, "Test executable has no directory");
  result.resize(slash + 1);
  return result;
}

HMODULE load_tip() {
  const auto directory = module_directory();
  for (const wchar_t *name : {L"libMetasequoiaImeTsf.dll", L"MetasequoiaImeTsf.dll"}) {
    const auto path = directory + name;
    if (auto module = LoadLibraryW(path.c_str()))
      return module;
  }
  throw std::runtime_error("Could not load the built TSF DLL");
}
} // namespace

int main() {
  bool initialized = false;
  try {
    initialized = SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED));
    require(initialized,
            "COM initialization failed");
    HMODULE module = load_tip();
    const auto raw_get_class_object = GetProcAddress(module, "DllGetClassObject");
    const auto raw_can_unload_now = GetProcAddress(module, "DllCanUnloadNow");
    DllGetClassObjectFn get_class_object = nullptr;
    DllCanUnloadNowFn can_unload_now = nullptr;
    static_assert(sizeof(get_class_object) == sizeof(raw_get_class_object));
    static_assert(sizeof(can_unload_now) == sizeof(raw_can_unload_now));
    std::memcpy(&get_class_object, &raw_get_class_object, sizeof(get_class_object));
    std::memcpy(&can_unload_now, &raw_can_unload_now, sizeof(can_unload_now));
    require(get_class_object != nullptr, "TSF DLL omitted DllGetClassObject");
    require(can_unload_now != nullptr, "TSF DLL omitted DllCanUnloadNow");

    require(can_unload_now() == S_OK, "Fresh TSF DLL must be unloadable");
    require(get_class_object(kUnknownClsid, IID_IClassFactory, nullptr) == E_INVALIDARG,
            "DllGetClassObject must reject a null output pointer");

    void *unknown_clsid_result = nullptr;
    require(get_class_object(kUnknownClsid, IID_IClassFactory, &unknown_clsid_result) ==
                CLASS_E_CLASSNOTAVAILABLE &&
                unknown_clsid_result == nullptr,
            "Unknown CLSID must be rejected without returning an interface");

    void *unsupported_result = nullptr;
    require(get_class_object(kMetasequoiaImeClsid, IID_ITfTextInputProcessor, &unsupported_result) ==
                E_NOINTERFACE &&
                unsupported_result == nullptr,
            "Known CLSID must reject an unsupported class-factory interface");

    IClassFactory *factory = nullptr;
    require(SUCCEEDED(get_class_object(kMetasequoiaImeClsid, IID_IClassFactory,
                                       reinterpret_cast<void **>(&factory))) &&
                factory != nullptr,
            "TSF class factory could not be created");
    require(can_unload_now() == S_FALSE,
            "TSF DLL must remain loaded while its class factory is referenced");
    require(factory->QueryInterface(IID_IClassFactory, nullptr) == E_POINTER,
            "Class factory QueryInterface must reject a null output pointer");

    IClassFactory *factory_alias = nullptr;
    require(SUCCEEDED(factory->QueryInterface(IID_IClassFactory,
                                               reinterpret_cast<void **>(&factory_alias))) &&
                factory_alias != nullptr,
            "Class factory QueryInterface could not return a second interface");
    require(can_unload_now() == S_FALSE,
            "TSF DLL must remain loaded while a queried class-factory interface is referenced");
    factory_alias->Release();

    require(factory->CreateInstance(nullptr, IID_IUnknown, nullptr) == E_INVALIDARG,
            "Class factory CreateInstance must reject a null output pointer");
    require(factory->LockServer(TRUE) == S_OK && can_unload_now() == S_FALSE,
            "LockServer(TRUE) must hold the TSF DLL loaded");
    require(factory->LockServer(FALSE) == S_OK && can_unload_now() == S_FALSE,
            "LockServer(FALSE) must release only its server lock");

    IUnknown *aggregated = nullptr;
    require(factory->CreateInstance(reinterpret_cast<IUnknown *>(factory), IID_IUnknown,
                                    reinterpret_cast<void **>(&aggregated)) == CLASS_E_NOAGGREGATION &&
                aggregated == nullptr,
            "TIP class factory must reject aggregation");

    IUnknown *unknown = nullptr;
    require(SUCCEEDED(factory->CreateInstance(nullptr, IID_IUnknown,
                                               reinterpret_cast<void **>(&unknown))) &&
                unknown != nullptr,
            "TSF class factory could not instantiate the TIP");
    ITfTextInputProcessor *processor = nullptr;
    require(SUCCEEDED(unknown->QueryInterface(IID_ITfTextInputProcessor,
                                               reinterpret_cast<void **>(&processor))) &&
                processor != nullptr,
            "TIP object does not implement ITfTextInputProcessor");
    require(can_unload_now() == S_FALSE,
            "TSF DLL must remain loaded while its TIP object is referenced");

    processor->Release();
    unknown->Release();
    require(can_unload_now() == S_FALSE,
            "TSF DLL must remain loaded while its class factory is referenced");
    factory->Release();
    require(can_unload_now() == S_OK,
            "TSF DLL must become unloadable after all COM references are released");
    FreeLibrary(module);
    CoUninitialize();
    initialized = false;
    return 0;
  } catch (...) {
    if (initialized)
      CoUninitialize();
    return 1;
  }
}
