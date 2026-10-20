#include "SystemAccent.h"

#include <iterator>

namespace msime::windows {
namespace {
// Windows.UI.Color。
struct UiColor {
  BYTE A;
  BYTE R;
  BYTE G;
  BYTE B;
};
// Windows.UI.ViewManagement.IUISettings3，只声明用到的 GetColorValue，它是 IInspectable 之后的第一个方法（后面是 add_ColorValuesChanged 和 remove_ColorValuesChanged，用不到就不声明）。按 Windows SDK 元数据自己声明、不包含 WinRT 头文件，理由与 msimeui 的 CompositionInterop.h 相同：MinGW 和 MSVC 两种工具链编同一份代码。`color` 是 UIColorType 枚举，ABI 上是 32 位整数。
struct IUISettings3 : IInspectable {
  virtual HRESULT STDMETHODCALLTYPE GetColorValue(int color, UiColor *value) = 0;
};
constexpr GUID kIidUISettings3{0x03021be4, 0x5254, 0x4781, {0x81, 0x94, 0x51, 0x68, 0xf7, 0xd0, 0x6d, 0x7b}};
// UIColorType::Accent。
constexpr int kUIColorTypeAccent = 5;

// 用到的 WinRT 运行时函数，在运行时从 combase.dll 取，不往 Server 的链接里加 runtimeobject，与 msimeui 的 DeviceResources.cpp 一样。
struct WinRtActivation {
  using ActivateInstance = HRESULT(WINAPI *)(HSTRING, IInspectable **);
  using CreateStringReference = HRESULT(WINAPI *)(PCWSTR, UINT32, HSTRING_HEADER *, HSTRING *);
  ActivateInstance activate = nullptr;
  CreateStringReference reference = nullptr;
};
// 经 void* 转换：GetProcAddress 返回通用的 FARPROC，直接转成真实签名会被 -Wcast-function-type 拒绝。
template <class Function> Function load_function(HMODULE module, const char *name) {
  return module ? reinterpret_cast<Function>(reinterpret_cast<void *>(GetProcAddress(module, name)))
                : nullptr;
}
const WinRtActivation &winrt_activation() {
  static const WinRtActivation functions = [] {
    WinRtActivation loaded;
    const HMODULE combase = LoadLibraryExW(L"combase.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
    loaded.activate = load_function<WinRtActivation::ActivateInstance>(combase, "RoActivateInstance");
    loaded.reference =
        load_function<WinRtActivation::CreateStringReference>(combase, "WindowsCreateStringReference");
    return loaded;
  }();
  return functions;
}
} // namespace

std::optional<uint32_t> SystemAccent::rgb() {
  if (!settings_ && !unavailable_) {
    unavailable_ = true;
    const auto &functions = winrt_activation();
    static constexpr wchar_t name[] = L"Windows.UI.ViewManagement.UISettings";
    HSTRING_HEADER header{};
    HSTRING runtime_class = nullptr;
    if (functions.activate && functions.reference &&
        SUCCEEDED(functions.reference(name, static_cast<UINT32>(std::size(name) - 1), &header,
                                      &runtime_class)) &&
        SUCCEEDED(functions.activate(runtime_class, settings_.ReleaseAndGetAddressOf())) && settings_)
      unavailable_ = false;
    else
      settings_.Reset();
  }
  if (!settings_)
    return std::nullopt;
  Microsoft::WRL::ComPtr<IUISettings3> colors;
  UiColor color{};
  if (FAILED(settings_->QueryInterface(kIidUISettings3, reinterpret_cast<void **>(colors.GetAddressOf()))) ||
      !colors || FAILED(colors->GetColorValue(kUIColorTypeAccent, &color)))
    return std::nullopt;
  return (static_cast<uint32_t>(color.R) << 16) | (static_cast<uint32_t>(color.G) << 8) |
         static_cast<uint32_t>(color.B);
}
} // namespace msime::windows
