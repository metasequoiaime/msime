#include "msimeui/DeviceResources.h"
#include "../../compat/WindowsDpi.h"

#include "CompositionInterop.h"
#include "msimeui/Fonts.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdio>
#include <cwchar>
#include <dwmapi.h>
#include <new>
#include <utility>
#include <windows.h>

namespace msimeui
{
namespace abi = composition;
using Microsoft::WRL::ComPtr;

struct DeviceResources::BackdropComposition
{
    ComPtr<abi::ICompositor> compositor;
    ComPtr<abi::IDesktopWindowTarget> target;
    // 系统材质：一个精灵视觉对象，画刷是模糊的来源叠亮度层和色调，按圆角矩形裁剪。
    ComPtr<abi::IVisual> backdrop;
    ComPtr<abi::ISpriteVisual> backdropSprite;
    ComPtr<abi::ICompositionRoundedRectangleGeometry> backdropShape;
    // BackdropMaterial::Source 的两个来源；这台 Windows 没有壁纸模糊时 wallpaper 为空。
    ComPtr<abi::ICompositionBrush> windowsBehind;
    ComPtr<abi::ICompositionBrush> wallpaper;
    // 内容：Present 把离屏位图拷进这个绘制表面，按像素原样左上对齐显示在材质上面。
    ComPtr<abi::IVisual> content;
    ComPtr<abi::ICompositionGraphicsDevice> graphicsDevice;
    ComPtr<abi::ICompositionDrawingSurfaceInterop> surface;
    // GetRenderTarget() 画进去的离屏位图，和绘制表面一样大。
    ComPtr<ID2D1Bitmap1> canvas;
    UINT width = 0;
    UINT height = 0;
};

namespace
{
UINT AlignSwapExtent(UINT value)
{
    constexpr UINT kBucket = 64;
    value = (std::max)(value, 1U);
    return ((value + kBucket - 1U) / kBucket) * kBucket;
}

// DWMWA_USE_HOSTBACKDROPBRUSH：Windows 11 要窗口自己声明用 host backdrop，HostBackdropBrush 才画得出模糊；旧的 SDK 和 MinGW 没有这个名字。Windows 10 上设不上也无妨，那里不需要它。
constexpr DWORD kDwmUseHostBackdropBrush = 17;

// E_BOUNDS：下标越界。Debian trixie 的 MinGW（12.0）的 winerror.h 还没有这个名字。
constexpr HRESULT kBoundsError = static_cast<HRESULT>(0x8000000BL);

constexpr GUID kIIDUnknown = {0x00000000, 0x0000, 0x0000, {0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46}};
constexpr GUID kIIDInspectable = {0xaf86e2e0, 0xb12d, 0x4c6a, {0x9c, 0x5a, 0xd7, 0xaa, 0x65, 0x10, 0x1e, 0x90}};

// 系统材质用到的 WinRT 运行时函数，在运行时从 combase.dll 取：不往链接里加 runtimeobject，取不到（不该发生在 Windows 8 以后）时材质模式建不起来，候选窗退回不透明底色。
struct WinRtFunctions
{
    using ActivateInstance = HRESULT(WINAPI *)(HSTRING, IInspectable **);
    using GetActivationFactory = HRESULT(WINAPI *)(HSTRING, REFIID, void **);
    using CreateStringReference = HRESULT(WINAPI *)(PCWSTR, UINT32, HSTRING_HEADER *, HSTRING *);
    ActivateInstance activate = nullptr;
    GetActivationFactory factory = nullptr;
    CreateStringReference reference = nullptr;
    bool available() const
    {
        return activate && factory && reference;
    }
};

// 经 void* 转换：GetProcAddress 返回通用的 FARPROC，直接转成真实签名会被 -Wcast-function-type 拒绝。
template <class Function> Function LoadFunction(HMODULE module, const char *name)
{
    return module ? reinterpret_cast<Function>(reinterpret_cast<void *>(GetProcAddress(module, name))) : nullptr;
}

// Win7 没有 dcomp.dll；静态导入会让整个 Server 在分层窗口回退之前就装载失败。系统支持时仍调用同一个入口，缺失时由 EnsureForComposition 选择原有回退。
HRESULT CreateCompositionDevice(IDXGIDevice *device, REFIID iid, void **result)
{
    using Create = HRESULT(WINAPI *)(IDXGIDevice *, REFIID, void **);
    static const Create create = LoadFunction<Create>(
        LoadLibraryExW(L"dcomp.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32), "DCompositionCreateDevice");
    if (!create)
    {
        if (result)
            *result = nullptr;
        return HRESULT_FROM_WIN32(ERROR_MOD_NOT_FOUND);
    }
    return create(device, iid, result);
}

const WinRtFunctions &WinRt()
{
    static const WinRtFunctions functions = [] {
        WinRtFunctions loaded;
        const HMODULE combase = LoadLibraryExW(L"combase.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
        loaded.activate = LoadFunction<WinRtFunctions::ActivateInstance>(combase, "RoActivateInstance");
        loaded.factory = LoadFunction<WinRtFunctions::GetActivationFactory>(combase, "RoGetActivationFactory");
        loaded.reference = LoadFunction<WinRtFunctions::CreateStringReference>(combase, "WindowsCreateStringReference");
        return loaded;
    }();
    return functions;
}

// 指向常量字符串的 HSTRING，不复制、不用释放；只在这个对象活着时有效，所以不能复制或移动。
class StringReference
{
  public:
    explicit StringReference(const wchar_t *text)
    {
        if (WinRt().reference)
        {
            WinRt().reference(text, static_cast<UINT32>(std::wcslen(text)), &header_, &value_);
        }
    }
    StringReference(const StringReference &) = delete;
    StringReference &operator=(const StringReference &) = delete;
    HSTRING Get() const
    {
        return value_;
    }

  private:
    HSTRING_HEADER header_ = {};
    HSTRING value_ = nullptr;
};

template <class Interface> HRESULT Query(IUnknown *from, const GUID &iid, ComPtr<Interface> &to)
{
    return from ? from->QueryInterface(iid, reinterpret_cast<void **>(to.ReleaseAndGetAddressOf())) : E_POINTER;
}

// Windows.UI.Composition 要求创建合成器的线程有一个 DispatcherQueue，一个线程只能有一个。控制器和线程同寿，故意不释放。CreateDispatcherQueueController 在 CoreMessaging.dll（Windows 10 1709 起），也在运行时取。
bool EnsureThreadDispatcherQueue()
{
    thread_local bool attempted = false;
    thread_local bool available = false;
    if (attempted)
    {
        return available;
    }
    attempted = true;
    struct Options
    {
        DWORD size;
        int threadType;
        int apartmentType;
    };
    using Create = HRESULT(WINAPI *)(Options, IInspectable **);
    const auto create = LoadFunction<Create>(
        LoadLibraryExW(L"CoreMessaging.dll", nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32), "CreateDispatcherQueueController");
    if (!create)
    {
        return false;
    }
    // DQTYPE_THREAD_CURRENT、DQTAT_COM_NONE：用这个线程自己的消息循环，不碰它已经初始化好的 COM 套间。
    const Options options{sizeof(Options), 2, 0};
    IInspectable *controller = nullptr;
    const HRESULT hr = create(options, &controller);
    // 线程已经有队列时失败也无妨，合成器用那一个。
    available = SUCCEEDED(hr) || hr == RPC_E_WRONG_THREAD || hr == HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
    return available;
}

// 交给合成器编译成效果画刷的一个 Direct2D 效果：合成器读它的 CLSID、按下标读属性和输入。没有动画，不需要按名字映射的属性。
class CompositionEffect final : public abi::IGraphicsEffect,
                                public abi::IGraphicsEffectSource,
                                public abi::IGraphicsEffectD2D1Interop
{
  public:
    CompositionEffect(const GUID &id, std::vector<ComPtr<abi::IPropertyValue>> properties,
                      std::vector<ComPtr<abi::IGraphicsEffectSource>> sources)
        : id_(id), properties_(std::move(properties)), sources_(std::move(sources))
    {
    }

    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void **object) override
    {
        if (!object)
        {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualGUID(iid, kIIDUnknown) || IsEqualGUID(iid, kIIDInspectable) ||
            IsEqualGUID(iid, abi::IID_IGraphicsEffect))
        {
            *object = static_cast<abi::IGraphicsEffect *>(this);
        }
        else if (IsEqualGUID(iid, abi::IID_IGraphicsEffectSource))
        {
            *object = static_cast<abi::IGraphicsEffectSource *>(this);
        }
        else if (IsEqualGUID(iid, abi::IID_IGraphicsEffectD2D1Interop))
        {
            *object = static_cast<abi::IGraphicsEffectD2D1Interop *>(this);
        }
        else
        {
            return E_NOINTERFACE;
        }
        AddRef();
        return S_OK;
    }
    ULONG STDMETHODCALLTYPE AddRef() override
    {
        return ++references_;
    }
    ULONG STDMETHODCALLTYPE Release() override
    {
        const ULONG remaining = --references_;
        if (remaining == 0)
        {
            delete this;
        }
        return remaining;
    }
    HRESULT STDMETHODCALLTYPE GetIids(ULONG *count, IID **iids) override
    {
        if (!count || !iids)
        {
            return E_POINTER;
        }
        *count = 0;
        *iids = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetRuntimeClassName(HSTRING *name) override
    {
        if (!name)
        {
            return E_POINTER;
        }
        *name = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetTrustLevel(TrustLevel *level) override
    {
        if (!level)
        {
            return E_POINTER;
        }
        *level = BaseTrust;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE get_Name(HSTRING *name) override
    {
        if (!name)
        {
            return E_POINTER;
        }
        *name = nullptr;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE put_Name(HSTRING) override
    {
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetEffectId(GUID *id) override
    {
        if (!id)
        {
            return E_POINTER;
        }
        *id = id_;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetNamedPropertyMapping(LPCWSTR, UINT *, int *) override
    {
        return E_INVALIDARG;
    }
    HRESULT STDMETHODCALLTYPE GetPropertyCount(UINT *count) override
    {
        if (!count)
        {
            return E_POINTER;
        }
        *count = static_cast<UINT>(properties_.size());
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetProperty(UINT index, abi::IPropertyValue **value) override
    {
        if (!value)
        {
            return E_POINTER;
        }
        return index < properties_.size() ? properties_[index].CopyTo(value) : kBoundsError;
    }
    HRESULT STDMETHODCALLTYPE GetSource(UINT index, abi::IGraphicsEffectSource **source) override
    {
        if (!source)
        {
            return E_POINTER;
        }
        return index < sources_.size() ? sources_[index].CopyTo(source) : kBoundsError;
    }
    HRESULT STDMETHODCALLTYPE GetSourceCount(UINT *count) override
    {
        if (!count)
        {
            return E_POINTER;
        }
        *count = static_cast<UINT>(sources_.size());
        return S_OK;
    }

  private:
    std::atomic<ULONG> references_{1};
    GUID id_;
    std::vector<ComPtr<abi::IPropertyValue>> properties_;
    std::vector<ComPtr<abi::IGraphicsEffectSource>> sources_;
};

ComPtr<abi::IGraphicsEffectSource> MakeEffect(const GUID &id, std::vector<ComPtr<abi::IPropertyValue>> properties,
                                              std::vector<ComPtr<abi::IGraphicsEffectSource>> sources)
{
    ComPtr<abi::IGraphicsEffectSource> result;
    for (const auto &property : properties)
    {
        if (!property)
        {
            return result;
        }
    }
    for (const auto &source : sources)
    {
        if (!source)
        {
            return result;
        }
    }
    // 引用计数从 1 起，交给 ComPtr 接管。
    if (auto *effect = new (std::nothrow) CompositionEffect(id, std::move(properties), std::move(sources)))
    {
        result.Attach(static_cast<abi::IGraphicsEffectSource *>(effect));
    }
    return result;
}
} // namespace

DeviceResources::DeviceResources() = default;

DeviceResources::~DeviceResources()
{
    // 合成器的对象先于它们绘制所用的 Direct3D 设备释放。
    DiscardTarget();
}

bool DeviceResources::IsDeviceLost(HRESULT hr)
{
    // 先转成 HRESULT 再比较：这几个宏在某些 SDK 和 MinGW 里是无符号的，直接和有符号的 HRESULT 比较会在一种工具链上报警告、在另一种上悄悄比错。
    return hr == static_cast<HRESULT>(D2DERR_RECREATE_TARGET) || hr == static_cast<HRESULT>(DXGI_ERROR_DEVICE_REMOVED) ||
           hr == static_cast<HRESULT>(DXGI_ERROR_DEVICE_RESET);
}

bool DeviceResources::IsSameColor(const D2D1_COLOR_F &lhs, const D2D1_COLOR_F &rhs)
{
    return lhs.r == rhs.r && lhs.g == rhs.g && lhs.b == rhs.b && lhs.a == rhs.a;
}

FLOAT DeviceResources::DpiForHwnd() const
{
    // The override wins over the system window DPI: window sizing and
    // rendering must share one scale even when GetDpiForWindow disagrees
    // with the content's real scale (RDP client-scaling sync).
    if (dpiOverride_ > 0.0f)
    {
        return dpiOverride_;
    }
    if (!hwnd_)
    {
        return 96.0f;
    }
    const UINT dpi = msime::windows::compat::window_dpi(hwnd_);
    return dpi > 0 ? static_cast<FLOAT>(dpi) : 96.0f;
}

void DeviceResources::SetDpiOverride(FLOAT dpi)
{
    const FLOAT normalized = dpi > 0.0f ? dpi : 0.0f;
    if (normalized == dpiOverride_)
    {
        return;
    }
    dpiOverride_ = normalized;
    // EnsureForComposition early-returns when the existing swap chain already
    // covers the requested size, so a live target would keep the stale DPI.
    // Push the new DPI into live targets right away; freshly created targets
    // read it through DpiForHwnd() instead.
    const FLOAT effective = DpiForHwnd();
    if (hwndRenderTarget_)
    {
        hwndRenderTarget_->SetDpi(effective, effective);
    }
    if (deviceContext_)
    {
        deviceContext_->SetDpi(effective, effective);
    }
}

bool DeviceResources::EnsureFactories()
{
    if (!d2dFactory_)
    {
        Microsoft::WRL::ComPtr<ID2D1Factory1> factory1;
        if (SUCCEEDED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory1.GetAddressOf())))
        {
            d2dFactory1_ = factory1;
            d2dFactory_ = factory1;
        }
        else if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, d2dFactory_.GetAddressOf())))
        {
            return false;
        }
    }

    if (!dwriteFactory_)
    {
        if (FAILED(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                                       reinterpret_cast<IUnknown **>(dwriteFactory_.GetAddressOf()))))
        {
            return false;
        }
    }

    // WIC is only ever used to decode bitmaps, and it is the one factory here
    // that goes through COM. A thread that never called CoInitialize gets
    // CO_E_NOTINITIALIZED, and failing the whole function for it stops surfaces
    // that draw nothing but text and shapes from drawing at all - which is how
    // the candidate flyout ended up invisible. Both bitmap getters already
    // check for a null factory and return nothing, so leave it null and let
    // them try again later, once whoever owns the thread has initialised COM.
    EnsureImagingFactory();
    return true;
}

bool DeviceResources::EnsureImagingFactory()
{
    if (wicFactory_)
    {
        return true;
    }
    return SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                      IID_PPV_ARGS(wicFactory_.GetAddressOf())));
}

bool DeviceResources::EnsureForWindow(HWND hwnd)
{
    hwnd_ = hwnd;
    composition_ = false;
    if (!EnsureFactories())
    {
        return false;
    }
    if (hwndRenderTarget_)
    {
        // The target is retained across monitor and resolution changes, and it
        // keeps whatever DPI it was created with. Refresh it here or every
        // later frame maps its logical coordinates against the old scale.
        const FLOAT dpi = DpiForHwnd();
        hwndRenderTarget_->SetDpi(dpi, dpi);
        return true;
    }

    DiscardTarget();
    RECT rc = {};
    GetClientRect(hwnd, &rc);
    pixelWidth_ = static_cast<UINT>((std::max)(rc.right, 1L));
    pixelHeight_ = static_cast<UINT>((std::max)(rc.bottom, 1L));
    const auto size = D2D1::SizeU(pixelWidth_, pixelHeight_);
    if (FAILED(d2dFactory_->CreateHwndRenderTarget(D2D1::RenderTargetProperties(),
                                                   D2D1::HwndRenderTargetProperties(hwnd, size),
                                                   hwndRenderTarget_.GetAddressOf())))
    {
        return false;
    }

    const FLOAT dpi = DpiForHwnd();
    hwndRenderTarget_->SetDpi(dpi, dpi);
    return true;
}

bool DeviceResources::BindCompositionSurface()
{
    if (!swapChain_ || !deviceContext_)
    {
        return false;
    }

    deviceContext_->SetTarget(nullptr);
    dxgiBitmap_.Reset();

    Microsoft::WRL::ComPtr<IDXGISurface> surface;
    if (FAILED(swapChain_->GetBuffer(0, IID_PPV_ARGS(surface.GetAddressOf()))))
    {
        return false;
    }

    const FLOAT dpi = DpiForHwnd();
    const D2D1_BITMAP_PROPERTIES1 props =
        D2D1::BitmapProperties1(D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                                D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), dpi, dpi);
    if (FAILED(deviceContext_->CreateBitmapFromDxgiSurface(surface.Get(), &props, dxgiBitmap_.GetAddressOf())))
    {
        return false;
    }
    deviceContext_->SetTarget(dxgiBitmap_.Get());
    deviceContext_->SetDpi(dpi, dpi);
    return true;
}

bool DeviceResources::EnsureForComposition(HWND hwnd)
{
    // 材质模式不退回分层窗口：分层窗口垫不了系统材质，由调用方关掉材质模式、画不透明底色。
    if (backdropComposition_)
    {
        return EnsureBackdropSurface(hwnd);
    }
    if (EnsureCompositionSurface(hwnd))
    {
        return true;
    }
    // DirectComposition is not everywhere: Wine implements it as a stub and
    // remote sessions can refuse it. The Windows source draws these surfaces as
    // layered windows, which gives the same per-pixel alpha without a
    // compositor, so fall back to that rather than reporting the window
    // unusable. Nothing reaches here on a host where composition worked.
    RECT rc = {};
    if (!hwnd || !GetClientRect(hwnd, &rc))
    {
        return false;
    }
    return EnsureLayered(hwnd, static_cast<UINT>((std::max)(rc.right, 1L)),
                         static_cast<UINT>((std::max)(rc.bottom, 1L)));
}

bool DeviceResources::EnsureCompositionSurface(HWND hwnd)
{
    hwnd_ = hwnd;
    if (!EnsureFactories() || !d2dFactory1_ || !hwnd)
    {
        return false;
    }

    RECT rc = {};
    GetClientRect(hwnd, &rc);
    const UINT width = AlignSwapExtent(static_cast<UINT>((std::max)(rc.right, 1L)));
    const UINT height = AlignSwapExtent(static_cast<UINT>((std::max)(rc.bottom, 1L)));

    if (composition_ && deviceContext_ && swapChain_ && pixelWidth_ >= width && pixelHeight_ >= height)
    {
        // Reusing the swap chain skips BindCompositionSurface, which is the
        // only other place the context's DPI is set. A window that moved to a
        // differently scaled monitor would otherwise keep drawing at the old
        // scale for as long as the surface stays big enough.
        const FLOAT dpi = DpiForHwnd();
        deviceContext_->SetDpi(dpi, dpi);
        return true;
    }

    if (!composition_ || !d3dDevice_ || !swapChain_)
    {
        DiscardTarget();
        composition_ = true;

        if (!CreateRenderingDevice())
        {
            composition_ = false;
            return false;
        }

        Microsoft::WRL::ComPtr<IDXGIAdapter> adapter;
        Microsoft::WRL::ComPtr<IDXGIFactory2> factory;
        if (FAILED(dxgiDevice_->GetAdapter(adapter.GetAddressOf())) ||
            FAILED(adapter->GetParent(IID_PPV_ARGS(factory.GetAddressOf()))))
        {
            composition_ = false;
            return false;
        }

        DXGI_SWAP_CHAIN_DESC1 desc = {};
        desc.Width = width;
        desc.Height = height;
        desc.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
        desc.SampleDesc.Count = 1;
        desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
        desc.BufferCount = 2;
        desc.SwapEffect = DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL;
        desc.AlphaMode = DXGI_ALPHA_MODE_PREMULTIPLIED;
        desc.Scaling = DXGI_SCALING_STRETCH;
        if (FAILED(factory->CreateSwapChainForComposition(d3dDevice_.Get(), &desc, nullptr, swapChain_.GetAddressOf())))
        {
            composition_ = false;
            return false;
        }

        // 窗口刚从材质模式切回来时，合成器的桌面窗口目标可能还占着最上层那个位置，退而用另一个；这个窗口没有重定向位图，两个位置画出来一样。
        if (FAILED(CreateCompositionDevice(dxgiDevice_.Get(), IID_PPV_ARGS(dcompDevice_.GetAddressOf()))) ||
            (FAILED(dcompDevice_->CreateTargetForHwnd(hwnd, TRUE, dcompTarget_.GetAddressOf())) &&
             FAILED(dcompDevice_->CreateTargetForHwnd(hwnd, FALSE, dcompTarget_.GetAddressOf()))) ||
            FAILED(dcompDevice_->CreateVisual(dcompVisual_.GetAddressOf())) ||
            FAILED(dcompVisual_->SetContent(swapChain_.Get())) || FAILED(dcompTarget_->SetRoot(dcompVisual_.Get())))
        {
            composition_ = false;
            return false;
        }
        pixelWidth_ = width;
        pixelHeight_ = height;
        if (!BindCompositionSurface())
        {
            composition_ = false;
            return false;
        }
        dcompDevice_->Commit();
        return true;
    }

    const UINT newWidth = (std::max)(pixelWidth_, width);
    const UINT newHeight = (std::max)(pixelHeight_, height);
    pixelWidth_ = newWidth;
    pixelHeight_ = newHeight;
    deviceContext_->SetTarget(nullptr);
    dxgiBitmap_.Reset();
    if (FAILED(swapChain_->ResizeBuffers(0, newWidth, newHeight, DXGI_FORMAT_B8G8R8A8_UNORM, 0)))
    {
        return false;
    }
    return BindCompositionSurface();
}

bool DeviceResources::CreateRenderingDevice()
{
    UINT flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
    D3D_FEATURE_LEVEL featureLevel = D3D_FEATURE_LEVEL_11_0;
    Microsoft::WRL::ComPtr<ID3D11DeviceContext> ignored;
    if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION,
                                 d3dDevice_.GetAddressOf(), &featureLevel, ignored.GetAddressOf())))
    {
        if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_WARP, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION,
                                     d3dDevice_.GetAddressOf(), &featureLevel, ignored.GetAddressOf())))
        {
            return false;
        }
    }
    if (FAILED(d3dDevice_.As(&dxgiDevice_)))
    {
        return false;
    }
    return SUCCEEDED(d2dFactory1_->CreateDevice(dxgiDevice_.Get(), d2dDevice_.GetAddressOf())) &&
           SUCCEEDED(d2dDevice_->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, deviceContext_.GetAddressOf()));
}

void DeviceResources::SetBackdropComposition(bool enabled)
{
    if (backdropComposition_ == enabled)
    {
        return;
    }
    backdropComposition_ = enabled;
    if (composition_)
    {
        DiscardTarget();
    }
}

bool DeviceResources::UsesBackdropComposition() const
{
    return backdropComposition_;
}

bool DeviceResources::BackdropWallpaperAvailable() const
{
    return backdropWallpaper_;
}

bool DeviceResources::BackdropStep(const char *step, HRESULT hr)
{
    if (FAILED(hr))
    {
        backdropFailureStep_ = step;
        backdropFailureResult_ = hr;
        return false;
    }
    return true;
}

std::string DeviceResources::BackdropFailure() const
{
    if (!backdropFailureStep_)
    {
        return {};
    }
    char result[16];
    std::snprintf(result, sizeof(result), "0x%08lX", static_cast<unsigned long>(backdropFailureResult_));
    return std::string(backdropFailureStep_) + " " + result;
}

bool DeviceResources::EnsureBackdropSurface(HWND hwnd)
{
    hwnd_ = hwnd;
    // 已经退成分层窗口的窗口不再试合成：那里建不起 DirectComposition，Windows.UI.Composition 也一样。
    if (backdropBroken_ || !hwnd || !BackdropStep("layered window", layered_ ? E_FAIL : S_OK) ||
        !BackdropStep("Direct2D factories", EnsureFactories() && d2dFactory1_ ? S_OK : E_FAIL) ||
        !BackdropStep("combase WinRT functions", WinRt().available() ? S_OK : E_NOTIMPL))
    {
        return false;
    }
    RECT rc = {};
    GetClientRect(hwnd, &rc);
    // 和交换链一样按 64 像素分档、只增不减，打字时卡片每宽一点不用重新分配。
    const UINT width = AlignSwapExtent(static_cast<UINT>((std::max)(rc.right, 1L)));
    const UINT height = AlignSwapExtent(static_cast<UINT>((std::max)(rc.bottom, 1L)));
    if (backdrop_ && deviceContext_)
    {
        if (backdrop_->width >= width && backdrop_->height >= height)
        {
            // 同交换链：复用表面时只有这里会把换了显示器之后的 DPI 交给设备上下文。
            const FLOAT dpi = DpiForHwnd();
            deviceContext_->SetDpi(dpi, dpi);
            return true;
        }
        const UINT grownWidth = (std::max)(backdrop_->width, width);
        const UINT grownHeight = (std::max)(backdrop_->height, height);
        if (!BackdropStep("Resize", backdrop_->surface->Resize(SIZE{static_cast<LONG>(grownWidth), static_cast<LONG>(grownHeight)})) ||
            !BackdropStep("put_Size", backdrop_->content->put_Size(
                {static_cast<float>(grownWidth), static_cast<float>(grownHeight)})))
        {
            DiscardTarget();
            return false;
        }
        backdrop_->width = grownWidth;
        backdrop_->height = grownHeight;
        if (!BackdropStep("offscreen bitmap", BindBackdropTarget() ? S_OK : E_FAIL))
        {
            DiscardTarget();
            return false;
        }
        return true;
    }

    DiscardTarget();
    composition_ = true;
    if (!BackdropStep("Direct3D device", CreateRenderingDevice() ? S_OK : E_FAIL) ||
        !BackdropStep("CreateDispatcherQueueController", EnsureThreadDispatcherQueue() ? S_OK : E_FAIL))
    {
        DiscardTarget();
        return false;
    }

    auto built = std::make_unique<BackdropComposition>();
    ComPtr<IInspectable> instance;
    ComPtr<abi::ICompositor3> compositor3;
    ComPtr<abi::ICompositor5> compositor5;
    ComPtr<abi::ICompositor6> compositor6;
    ComPtr<abi::ICompositorDesktopInterop> desktopInterop;
    ComPtr<abi::ICompositorInterop> interop;
    const StringReference compositorClass(L"Windows.UI.Composition.Compositor");
    if (!compositorClass.Get() || !BackdropStep("RoActivateInstance Compositor", WinRt().activate(compositorClass.Get(), instance.GetAddressOf())) ||
        !BackdropStep("QueryInterface ICompositor", Query(instance.Get(), abi::IID_ICompositor, built->compositor)) ||
        !BackdropStep("QueryInterface ICompositor3", Query(instance.Get(), abi::IID_ICompositor3, compositor3)) ||
        !BackdropStep("QueryInterface ICompositor5", Query(instance.Get(), abi::IID_ICompositor5, compositor5)) ||
        !BackdropStep("QueryInterface ICompositor6", Query(instance.Get(), abi::IID_ICompositor6, compositor6)) ||
        !BackdropStep("QueryInterface ICompositorDesktopInterop", Query(instance.Get(), abi::IID_ICompositorDesktopInterop, desktopInterop)) ||
        !BackdropStep("QueryInterface ICompositorInterop", Query(instance.Get(), abi::IID_ICompositorInterop, interop)))
    {
        DiscardTarget();
        return false;
    }
    // DiscardTarget 已经放掉了这个窗口可能有的 DirectComposition 目标，最上层的位置空了；还没空出来就用另一个。
    if (!BackdropStep("CreateDesktopWindowTarget", desktopInterop->CreateDesktopWindowTarget(hwnd, TRUE, built->target.GetAddressOf())) &&
        !BackdropStep("CreateDesktopWindowTarget", desktopInterop->CreateDesktopWindowTarget(hwnd, FALSE, built->target.GetAddressOf())))
    {
        DiscardTarget();
        return false;
    }

    ComPtr<abi::IContainerVisual> root;
    ComPtr<abi::IVisual> rootVisual;
    ComPtr<abi::ICompositionTarget> target;
    ComPtr<abi::IVisualCollection> children;
    ComPtr<abi::ICompositionBackdropBrush> hostBackdrop;
    ComPtr<abi::ICompositionGeometry> shapeGeometry;
    ComPtr<abi::ICompositionGeometricClip> geometricClip;
    ComPtr<abi::ICompositionClip> clip;
    ComPtr<abi::ICompositionDrawingSurface> drawingSurface;
    ComPtr<abi::ICompositionSurface> surface;
    ComPtr<abi::ICompositionSurfaceBrush> surfaceBrush;
    ComPtr<abi::ICompositionBrush> surfaceBrushBase;
    ComPtr<abi::ISpriteVisual> contentSprite;
    abi::ICompositor *compositor = built->compositor.Get();
    const abi::Size size{static_cast<float>(width), static_cast<float>(height)};
    if (!BackdropStep("CreateContainerVisual", compositor->CreateContainerVisual(root.GetAddressOf())) ||
        !BackdropStep("QueryInterface IVisual", Query(root.Get(), abi::IID_IVisual, rootVisual)) ||
        !BackdropStep("QueryInterface ICompositionTarget", Query(built->target.Get(), abi::IID_ICompositionTarget, target)) ||
        !BackdropStep("put_Root", target->put_Root(rootVisual.Get())) || !BackdropStep("get_Children", root->get_Children(children.GetAddressOf())) ||
        // 材质：窗口后面已经模糊好的画面，按圆角矩形裁剪，先藏起来，等 SetBackdrop 给出位置。
        !BackdropStep("CreateHostBackdropBrush", compositor3->CreateHostBackdropBrush(hostBackdrop.GetAddressOf())) ||
        !BackdropStep("QueryInterface ICompositionBrush", Query(hostBackdrop.Get(), abi::IID_ICompositionBrush, built->windowsBehind)) ||
        !BackdropStep("CreateSpriteVisual", compositor->CreateSpriteVisual(built->backdropSprite.GetAddressOf())) ||
        !BackdropStep("QueryInterface IVisual", Query(built->backdropSprite.Get(), abi::IID_IVisual, built->backdrop)) ||
        !BackdropStep("CreateRoundedRectangleGeometry", compositor5->CreateRoundedRectangleGeometry(built->backdropShape.GetAddressOf())) ||
        !BackdropStep("QueryInterface ICompositionGeometry", Query(built->backdropShape.Get(), abi::IID_ICompositionGeometry, shapeGeometry)) ||
        !BackdropStep("CreateGeometricClipWithGeometry", compositor6->CreateGeometricClipWithGeometry(shapeGeometry.Get(), geometricClip.GetAddressOf())) ||
        !BackdropStep("QueryInterface ICompositionClip", Query(geometricClip.Get(), abi::IID_ICompositionClip, clip)) ||
        !BackdropStep("put_Clip", built->backdrop->put_Clip(clip.Get())) || !BackdropStep("put_IsVisible", built->backdrop->put_IsVisible(static_cast<boolean>(false))) ||
        !BackdropStep("InsertAtBottom", children->InsertAtBottom(built->backdrop.Get())) ||
        // 内容：Direct2D 画进绘制表面，按像素原样、左上对齐。
        !BackdropStep("CreateGraphicsDevice", interop->CreateGraphicsDevice(d2dDevice_.Get(), built->graphicsDevice.GetAddressOf())) ||
        !BackdropStep("CreateDrawingSurface", built->graphicsDevice->CreateDrawingSurface(size, abi::kPixelFormatB8G8R8A8UIntNormalized,
                                                           abi::kAlphaModePremultiplied,
                                                           drawingSurface.GetAddressOf())) ||
        !BackdropStep("QueryInterface ICompositionSurface", Query(drawingSurface.Get(), abi::IID_ICompositionSurface, surface)) ||
        !BackdropStep("QueryInterface ICompositionDrawingSurfaceInterop", Query(drawingSurface.Get(), abi::IID_ICompositionDrawingSurfaceInterop, built->surface)) ||
        !BackdropStep("CreateSurfaceBrushWithSurface", compositor->CreateSurfaceBrushWithSurface(surface.Get(), surfaceBrush.GetAddressOf())) ||
        !BackdropStep("put_Stretch", surfaceBrush->put_Stretch(abi::kStretchNone)) ||
        !BackdropStep("put_HorizontalAlignmentRatio", surfaceBrush->put_HorizontalAlignmentRatio(0.0f)) ||
        !BackdropStep("put_VerticalAlignmentRatio", surfaceBrush->put_VerticalAlignmentRatio(0.0f)) ||
        !BackdropStep("QueryInterface ICompositionBrush", Query(surfaceBrush.Get(), abi::IID_ICompositionBrush, surfaceBrushBase)) ||
        !BackdropStep("CreateSpriteVisual", compositor->CreateSpriteVisual(contentSprite.GetAddressOf())) ||
        !BackdropStep("put_Brush", contentSprite->put_Brush(surfaceBrushBase.Get())) ||
        !BackdropStep("QueryInterface IVisual", Query(contentSprite.Get(), abi::IID_IVisual, built->content)) ||
        !BackdropStep("put_Size", built->content->put_Size({size.Width, size.Height})) ||
        !BackdropStep("InsertAtTop", children->InsertAtTop(built->content.Get())))
    {
        DiscardTarget();
        return false;
    }
    // 模糊的壁纸（云母的来源）是 Windows 11 才有的；没有时云母按窗口后面的画面画。
    ComPtr<abi::ICompositorWithBlurredWallpaperBackdropBrush> wallpaperCompositor;
    ComPtr<abi::ICompositionBackdropBrush> wallpaper;
    backdropWallpaper_ =
        SUCCEEDED(Query(instance.Get(), abi::IID_ICompositorWithBlurredWallpaperBackdropBrush, wallpaperCompositor)) &&
        SUCCEEDED(wallpaperCompositor->TryCreateBlurredWallpaperBackdropBrush(wallpaper.GetAddressOf())) &&
        wallpaper && SUCCEEDED(Query(wallpaper.Get(), abi::IID_ICompositionBrush, built->wallpaper));
    built->width = width;
    built->height = height;
    backdrop_ = std::move(built);
    if (!BackdropStep("offscreen bitmap", BindBackdropTarget() ? S_OK : E_FAIL) || !ApplyBackdropMaterial())
    {
        DiscardTarget();
        return false;
    }
    BOOL useHostBackdrop = TRUE;
    DwmSetWindowAttribute(hwnd, kDwmUseHostBackdropBrush, &useHostBackdrop, sizeof(useHostBackdrop));
    return true;
}

bool DeviceResources::BindBackdropTarget()
{
    if (!backdrop_ || !deviceContext_)
    {
        return false;
    }
    deviceContext_->SetTarget(nullptr);
    backdrop_->canvas.Reset();
    const FLOAT dpi = DpiForHwnd();
    const D2D1_BITMAP_PROPERTIES1 props = D2D1::BitmapProperties1(
        D2D1_BITMAP_OPTIONS_TARGET, D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED), dpi,
        dpi);
    if (FAILED(deviceContext_->CreateBitmap(D2D1::SizeU(backdrop_->width, backdrop_->height), nullptr, 0, &props,
                                            backdrop_->canvas.GetAddressOf())))
    {
        return false;
    }
    deviceContext_->SetTarget(backdrop_->canvas.Get());
    deviceContext_->SetDpi(dpi, dpi);
    return true;
}

void DeviceResources::SetBackdropMaterial(const BackdropMaterial &material)
{
    if (backdropMaterial_ == material)
    {
        return;
    }
    backdropMaterial_ = material;
    if (backdrop_ && !ApplyBackdropMaterial())
    {
        // 换不上新画刷：这次运行不再用材质模式，下一次 EnsureForComposition 返回 false，调用方退回不透明底色。
        backdropBroken_ = true;
        DiscardTarget();
    }
}

bool DeviceResources::ApplyBackdropMaterial()
{
    const BackdropMaterial &material = backdropMaterial_;
    const ComPtr<abi::ICompositionBrush> &source =
        material.source == BackdropMaterial::Source::Wallpaper && backdrop_->wallpaper ? backdrop_->wallpaper
                                                                                       : backdrop_->windowsBehind;
    if (material.tintOpacity <= 0.0f && material.luminosityOpacity <= 0.0f)
    {
        return BackdropStep("put_Brush", backdrop_->backdropSprite->put_Brush(source.Get()));
    }

    ComPtr<abi::IPropertyValueStatics> values;
    ComPtr<abi::ICompositionEffectSourceParameterFactory> parameters;
    ComPtr<abi::ICompositionEffectSourceParameter> parameter;
    ComPtr<abi::IGraphicsEffectSource> backdropInput;
    const StringReference propertyValueClass(L"Windows.Foundation.PropertyValue");
    const StringReference parameterClass(L"Windows.UI.Composition.CompositionEffectSourceParameter");
    const StringReference parameterName(L"Backdrop");
    if (!BackdropStep("PropertyValue factory",
                      WinRt().factory(propertyValueClass.Get(), abi::IID_IPropertyValueStatics,
                                      reinterpret_cast<void **>(values.GetAddressOf()))) ||
        !BackdropStep("CompositionEffectSourceParameter factory",
                      WinRt().factory(parameterClass.Get(), abi::IID_ICompositionEffectSourceParameterFactory,
                                      reinterpret_cast<void **>(parameters.GetAddressOf()))) ||
        !BackdropStep("CompositionEffectSourceParameter", parameters->Create(parameterName.Get(),
                                                                             parameter.GetAddressOf())) ||
        !BackdropStep("QueryInterface IGraphicsEffectSource",
                      Query(parameter.Get(), abi::IID_IGraphicsEffectSource, backdropInput)))
    {
        return false;
    }
    const auto box = [](HRESULT hr, const ComPtr<IInspectable> &boxed) {
        ComPtr<abi::IPropertyValue> value;
        if (SUCCEEDED(hr))
        {
            Query(boxed.Get(), abi::IID_IPropertyValue, value);
        }
        return value;
    };
    // 色调颜色在 alpha 处的纯色填充；Direct2D 的 Flood 取非预乘的颜色。
    const auto flood = [&](float alpha) {
        const FLOAT rgba[4] = {material.tint.r, material.tint.g, material.tint.b, std::clamp(alpha, 0.0f, 1.0f)};
        ComPtr<IInspectable> boxed;
        return MakeEffect(abi::CLSID_Flood, {box(values->CreateSingleArray(4, rgba, boxed.GetAddressOf()), boxed)},
                          {});
    };
    // Direct2D 混合效果的输入：0 是背景（目标），1 是前景（源）。
    const auto blend = [&](UINT32 mode, const ComPtr<abi::IGraphicsEffectSource> &background,
                           const ComPtr<abi::IGraphicsEffectSource> &foreground) {
        ComPtr<IInspectable> boxed;
        return MakeEffect(abi::CLSID_Blend, {box(values->CreateUInt32(mode, boxed.GetAddressOf()), boxed)},
                          {background, foreground});
    };
    // 与 WinUI 的 AcrylicBrush 同一张图。合成器把 Color 与 Luminosity 两种混合对调了，所以和 WinUI 一样，亮度那一步写 COLOR，色调那一步写 LUMINOSITY。
    const auto luminosity = blend(abi::kBlendModeColor, backdropInput, flood(material.luminosityOpacity));
    const auto tinted = blend(abi::kBlendModeLuminosity, luminosity, flood(material.tintOpacity));

    ComPtr<abi::IGraphicsEffect> effect;
    ComPtr<abi::ICompositionEffectFactory> factory;
    ComPtr<abi::ICompositionEffectBrush> effectBrush;
    ComPtr<abi::ICompositionBrush> brush;
    return BackdropStep("effect graph", tinted ? S_OK : E_OUTOFMEMORY) &&
           BackdropStep("QueryInterface IGraphicsEffect", Query(tinted.Get(), abi::IID_IGraphicsEffect, effect)) &&
           BackdropStep("CreateEffectFactory",
                        backdrop_->compositor->CreateEffectFactory(effect.Get(), factory.GetAddressOf())) &&
           BackdropStep("CreateBrush", factory->CreateBrush(effectBrush.GetAddressOf())) &&
           BackdropStep("SetSourceParameter", effectBrush->SetSourceParameter(parameterName.Get(), source.Get())) &&
           BackdropStep("QueryInterface ICompositionBrush", Query(effectBrush.Get(), abi::IID_ICompositionBrush, brush)) &&
           BackdropStep("put_Brush", backdrop_->backdropSprite->put_Brush(brush.Get()));
}

void DeviceResources::SetBackdrop(const D2D1_RECT_F &rect, float radius, float opacity)
{
    if (!backdrop_)
    {
        return;
    }
    const bool visible = rect.right > rect.left && rect.bottom > rect.top && opacity > 0.0f;
    backdrop_->backdrop->put_IsVisible(static_cast<boolean>(visible));
    if (!visible)
    {
        return;
    }
    // 窗口目标里的视觉对象按物理像素排。
    const float scale = DpiForHwnd() / 96.0f;
    const float left = std::round(rect.left * scale);
    const float top = std::round(rect.top * scale);
    const float right = std::round(rect.right * scale);
    const float bottom = std::round(rect.bottom * scale);
    const float corner = (std::max)(radius, 0.0f) * scale;
    backdrop_->backdrop->put_Offset({left, top, 0.0f});
    backdrop_->backdrop->put_Size({right - left, bottom - top});
    backdrop_->backdrop->put_Opacity((std::min)(opacity, 1.0f));
    backdrop_->backdropShape->put_Size({right - left, bottom - top});
    backdrop_->backdropShape->put_CornerRadius({corner, corner});
}

HRESULT DeviceResources::PresentBackdrop()
{
    if (!backdrop_->canvas)
    {
        return S_OK;
    }
    // 绘制表面不是设备丢失的错：这次运行不再用材质模式，回 D2DERR_RECREATE_TARGET 让调用方丢掉目标、重画，下一次 EnsureForComposition 返回 false，调用方退回不透明底色。
    const auto failed = [this](const char *step, HRESULT hr) {
        BackdropStep(step, hr);
        if (!IsDeviceLost(hr))
        {
            backdropBroken_ = true;
            hr = static_cast<HRESULT>(D2DERR_RECREATE_TARGET);
        }
        return hr;
    };
    // 绑在设备上下文上当目标的位图不能拿来当源画，拷贝期间先解开，拷完再绑回去。
    deviceContext_->SetTarget(nullptr);
    const auto rebind = [this] {
        deviceContext_->SetTarget(backdrop_->canvas.Get());
        const FLOAT dpi = DpiForHwnd();
        deviceContext_->SetDpi(dpi, dpi);
    };
    // 每帧整张表面重画，和交换链的缓冲区一样。
    const RECT update = {0, 0, static_cast<LONG>(backdrop_->width), static_cast<LONG>(backdrop_->height)};
    POINT offset = {};
    ComPtr<ID2D1DeviceContext> context;
    HRESULT hr = backdrop_->surface->BeginDraw(&update, IID_PPV_ARGS(context.GetAddressOf()), &offset);
    if (FAILED(hr))
    {
        rebind();
        return failed("BeginDraw", hr);
    }
    // 交给我们的上下文画进一张共享的图集，offset 是这块表面在图集里的起点（像素）。按 96 DPI 画，一个 DIP 就是一个像素；先裁到这块表面，Clear 才不会擦到别人。
    context->SetDpi(96.0f, 96.0f);
    context->SetTransform(D2D1::Matrix3x2F::Identity());
    const D2D1_RECT_F area = D2D1::RectF(static_cast<float>(offset.x), static_cast<float>(offset.y),
                                         static_cast<float>(offset.x) + static_cast<float>(backdrop_->width),
                                         static_cast<float>(offset.y) + static_cast<float>(backdrop_->height));
    context->PushAxisAlignedClip(area, D2D1_ANTIALIAS_MODE_ALIASED);
    context->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
    static_cast<ID2D1RenderTarget *>(context.Get())
        ->DrawBitmap(backdrop_->canvas.Get(), &area, 1.0f, D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR, nullptr);
    context->PopAxisAlignedClip();
    context.Reset();
    hr = backdrop_->surface->EndDraw();
    rebind();
    return FAILED(hr) ? failed("EndDraw", hr) : S_OK;
}

void DeviceResources::Resize(UINT width, UINT height)
{
    width = (std::max)(width, 1U);
    height = (std::max)(height, 1U);
    if (hwndRenderTarget_)
    {
        hwndRenderTarget_->Resize(D2D1::SizeU(width, height));
        pixelWidth_ = width;
        pixelHeight_ = height;
        return;
    }
    if (composition_ && swapChain_ && deviceContext_)
    {
        const UINT neededW = AlignSwapExtent(width);
        const UINT neededH = AlignSwapExtent(height);
        if (pixelWidth_ >= neededW && pixelHeight_ >= neededH)
        {
            return;
        }
        pixelWidth_ = (std::max)(pixelWidth_, neededW);
        pixelHeight_ = (std::max)(pixelHeight_, neededH);
        deviceContext_->SetTarget(nullptr);
        dxgiBitmap_.Reset();
        if (SUCCEEDED(swapChain_->ResizeBuffers(0, pixelWidth_, pixelHeight_, DXGI_FORMAT_B8G8R8A8_UNORM, 0)))
        {
            BindCompositionSurface();
        }
    }
}

void DeviceResources::DiscardTarget()
{
    if (backdrop_)
    {
        // 显式关掉桌面窗口目标，让出窗口的合成目标位置，切回交换链时 DirectComposition 才建得起目标。
        ComPtr<abi::IClosable> closable;
        if (SUCCEEDED(Query(backdrop_->target.Get(), abi::IID_IClosable, closable)))
        {
            closable->Close();
        }
        backdrop_.reset();
    }
    if (deviceContext_)
    {
        deviceContext_->SetTarget(nullptr);
    }
    dxgiBitmap_.Reset();
    hwndRenderTarget_.Reset();
    deviceContext_.Reset();
    d2dDevice_.Reset();
    swapChain_.Reset();
    dcompVisual_.Reset();
    dcompTarget_.Reset();
    dcompDevice_.Reset();
    dxgiDevice_.Reset();
    d3dDevice_.Reset();
    brushCache_.clear();
    bitmapCache_.clear();
    composition_ = false;
}

bool DeviceResources::EnsureLayered(HWND hwnd, UINT width, UINT height)
{
    // Only Direct2D and the DC target are needed here; the composition path's
    // ID2D1Factory1 is not.
    if (!EnsureFactories() || !d2dFactory_)
    {
        return false;
    }
    hwnd_ = hwnd;
    if (layered_ && dcTarget_ && layeredWidth_ == width && layeredHeight_ == height)
    {
        const FLOAT dpi = DpiForHwnd();
        dcTarget_->SetDpi(dpi, dpi);
        return true;
    }
    DiscardLayered();

    // Top-down and premultiplied: what UpdateLayeredWindow's ULW_ALPHA expects,
    // and what Direct2D writes with D2D1_ALPHA_MODE_PREMULTIPLIED.
    BITMAPINFO info = {};
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = static_cast<LONG>(width);
    info.bmiHeader.biHeight = -static_cast<LONG>(height);
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    const HDC screen = GetDC(nullptr);
    if (!screen)
    {
        return false;
    }
    void *bits = nullptr;
    layeredBitmap_ = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &bits, nullptr, 0);
    layeredDC_ = CreateCompatibleDC(screen);
    ReleaseDC(nullptr, screen);
    if (!layeredBitmap_ || !layeredDC_)
    {
        DiscardLayered();
        return false;
    }
    layeredPrevious_ = SelectObject(layeredDC_, layeredBitmap_);

    if (!dcTarget_)
    {
        const auto properties = D2D1::RenderTargetProperties(
            D2D1_RENDER_TARGET_TYPE_DEFAULT,
            D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_PREMULTIPLIED));
        if (FAILED(d2dFactory_->CreateDCRenderTarget(&properties, dcTarget_.GetAddressOf())))
        {
            DiscardLayered();
            return false;
        }
    }
    const RECT bind = {0, 0, static_cast<LONG>(width), static_cast<LONG>(height)};
    if (FAILED(dcTarget_->BindDC(layeredDC_, &bind)))
    {
        DiscardLayered();
        return false;
    }
    const FLOAT dpi = DpiForHwnd();
    dcTarget_->SetDpi(dpi, dpi);

    // The style is what makes UpdateLayeredWindow legal for this window. The
    // surfaces already create themselves WS_EX_NOACTIVATE; this adds to it.
    const LONG_PTR style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    if ((style & WS_EX_LAYERED) == 0)
    {
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED);
    }
    layeredWidth_ = width;
    layeredHeight_ = height;
    layered_ = true;
    return true;
}

void DeviceResources::DiscardLayered()
{
    if (layeredDC_)
    {
        if (layeredPrevious_)
        {
            SelectObject(layeredDC_, layeredPrevious_);
        }
        DeleteDC(layeredDC_);
    }
    if (layeredBitmap_)
    {
        DeleteObject(layeredBitmap_);
    }
    layeredPrevious_ = nullptr;
    layeredDC_ = nullptr;
    layeredBitmap_ = nullptr;
    layeredWidth_ = 0;
    layeredHeight_ = 0;
    layered_ = false;
}

HRESULT DeviceResources::Present()
{
    if (backdrop_)
    {
        return PresentBackdrop();
    }
    if (layered_)
    {
        SIZE size = {static_cast<LONG>(layeredWidth_), static_cast<LONG>(layeredHeight_)};
        POINT source = {0, 0};
        BLENDFUNCTION blend = {AC_SRC_OVER, 0, 255, AC_SRC_ALPHA};
        // No destination point: the surfaces position themselves, and supplying
        // one here would move the window out from under them.
        const HDC screen = GetDC(nullptr);
        const BOOL updated = UpdateLayeredWindow(hwnd_, screen, nullptr, &size, layeredDC_,
                                                 &source, 0, &blend, ULW_ALPHA);
        if (screen)
        {
            ReleaseDC(nullptr, screen);
        }
        return updated ? S_OK : E_FAIL;
    }
    if (!swapChain_)
    {
        return S_OK;
    }
    const HRESULT hr = swapChain_->Present(0, 0);
    if (dcompDevice_)
    {
        dcompDevice_->Commit();
    }
    return hr;
}

D2D1_SIZE_F DeviceResources::GetClientSize() const
{
    RECT rc = {};
    if (!hwnd_ || !GetClientRect(hwnd_, &rc))
    {
        auto *target = GetRenderTarget();
        return target ? target->GetSize() : D2D1::SizeF(0.0f, 0.0f);
    }
    // 与 BindCompositionSurface、EnsureLayered 给渲染目标设的 DPI 同一个来源，换算出的 DIP 和画图用的坐标一致。
    const FLOAT scale = 96.0f / DpiForHwnd();
    return D2D1::SizeF(static_cast<FLOAT>(rc.right) * scale, static_cast<FLOAT>(rc.bottom) * scale);
}

ID2D1RenderTarget *DeviceResources::GetRenderTarget() const
{
    if (layered_ && dcTarget_)
    {
        return dcTarget_.Get();
    }
    if (deviceContext_)
    {
        return deviceContext_.Get();
    }
    return hwndRenderTarget_.Get();
}

ID2D1DeviceContext *DeviceResources::GetDeviceContext() const
{
    return deviceContext_.Get();
}

bool DeviceResources::UsesComposition() const
{
    return composition_;
}

IDWriteFactory *DeviceResources::GetDWriteFactory() const
{
    return dwriteFactory_.Get();
}

ID2D1SolidColorBrush *DeviceResources::GetSolidColorBrush(const D2D1_COLOR_F &color)
{
    ID2D1RenderTarget *target = GetRenderTarget();
    if (!target)
    {
        return nullptr;
    }

    for (auto &entry : brushCache_)
    {
        if (entry.brush && IsSameColor(entry.color, color))
        {
            return entry.brush.Get();
        }
    }

    BrushCacheEntry entry;
    entry.color = color;
    if (FAILED(target->CreateSolidColorBrush(color, entry.brush.GetAddressOf())))
    {
        return nullptr;
    }

    brushCache_.push_back(std::move(entry));
    return brushCache_.back().brush.Get();
}

IDWriteTextFormat *DeviceResources::GetTextFormat(const std::wstring &fontFamily, float fontSize,
                                                  DWRITE_FONT_WEIGHT fontWeight, DWRITE_TEXT_ALIGNMENT textAlignment,
                                                  DWRITE_PARAGRAPH_ALIGNMENT paragraphAlignment,
                                                  DWRITE_WORD_WRAPPING wordWrapping)
{
    if (!dwriteFactory_)
    {
        return nullptr;
    }

    TextFormatKey key;
    key.fontFamily = fontFamily;
    key.fontSize = fontSize;
    key.fontWeight = fontWeight;
    key.textAlignment = textAlignment;
    key.paragraphAlignment = paragraphAlignment;
    key.wordWrapping = wordWrapping;
    if (Microsoft::WRL::ComPtr<IDWriteTextFormat> *cached = textFormatCache_.Find(key); cached && *cached)
    {
        return cached->Get();
    }

    Microsoft::WRL::ComPtr<IDWriteTextFormat> format;
    if (FAILED(dwriteFactory_->CreateTextFormat(key.fontFamily.c_str(), nullptr, key.fontWeight,
                                                DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, key.fontSize,
                                                L"", format.GetAddressOf())))
    {
        if (FAILED(dwriteFactory_->CreateTextFormat(L"Microsoft YaHei", nullptr, key.fontWeight,
                                                    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                                    key.fontSize, L"", format.GetAddressOf())))
        {
            return nullptr;
        }
    }

    format->SetTextAlignment(key.textAlignment);
    format->SetParagraphAlignment(key.paragraphAlignment);
    format->SetWordWrapping(key.wordWrapping);
    ApplyUiFontFallback(dwriteFactory_.Get(), format.Get());
    textFormatCache_.Insert(key, format);
    Microsoft::WRL::ComPtr<IDWriteTextFormat> *cached = textFormatCache_.Find(key);
    return cached ? cached->Get() : nullptr;
}

ID2D1Bitmap *DeviceResources::GetBitmapFromFile(const std::wstring &filePath, D2D1_SIZE_F *size)
{
    ID2D1RenderTarget *target = GetRenderTarget();
    if (!target || filePath.empty() || !EnsureImagingFactory())
    {
        return nullptr;
    }

    for (auto &entry : bitmapCache_)
    {
        if (entry.filePath == filePath && entry.bitmap)
        {
            if (size)
            {
                *size = entry.size;
            }
            return entry.bitmap.Get();
        }
    }

    Microsoft::WRL::ComPtr<IWICBitmapDecoder> decoder;
    if (FAILED(wicFactory_->CreateDecoderFromFilename(filePath.c_str(), nullptr, GENERIC_READ,
                                                      WICDecodeMetadataCacheOnLoad, decoder.GetAddressOf())))
    {
        return nullptr;
    }

    Microsoft::WRL::ComPtr<IWICBitmapFrameDecode> frame;
    if (FAILED(decoder->GetFrame(0, frame.GetAddressOf())))
    {
        return nullptr;
    }

    Microsoft::WRL::ComPtr<IWICFormatConverter> converter;
    if (FAILED(wicFactory_->CreateFormatConverter(converter.GetAddressOf())) ||
        FAILED(converter->Initialize(frame.Get(), GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, nullptr, 0.0,
                                     WICBitmapPaletteTypeMedianCut)))
    {
        return nullptr;
    }

    BitmapCacheEntry entry;
    entry.filePath = filePath;
    if (FAILED(target->CreateBitmapFromWicBitmap(converter.Get(), nullptr, entry.bitmap.GetAddressOf())))
    {
        return nullptr;
    }
    entry.size = entry.bitmap->GetSize();
    bitmapCache_.push_back(std::move(entry));
    if (size)
    {
        *size = bitmapCache_.back().size;
    }
    return bitmapCache_.back().bitmap.Get();
}

ID2D1Bitmap *DeviceResources::GetBitmapFromIcon(HICON icon, const std::wstring &key, D2D1_SIZE_F *size)
{
    ID2D1RenderTarget *target = GetRenderTarget();
    if (!target || !icon || key.empty() || !EnsureImagingFactory())
    {
        return nullptr;
    }

    // Shares the file cache: the key is the caller's, and an icon key and a
    // path cannot collide as long as callers keep using a prefix.
    for (auto &entry : bitmapCache_)
    {
        if (entry.filePath == key && entry.bitmap)
        {
            if (size)
            {
                *size = entry.size;
            }
            return entry.bitmap.Get();
        }
    }

    Microsoft::WRL::ComPtr<IWICBitmap> source;
    if (FAILED(wicFactory_->CreateBitmapFromHICON(icon, source.GetAddressOf())))
    {
        return nullptr;
    }

    // CreateBitmapFromHICON hands back straight alpha; Direct2D wants it
    // premultiplied, and drawing the unconverted bitmap haloes every edge.
    Microsoft::WRL::ComPtr<IWICFormatConverter> converter;
    if (FAILED(wicFactory_->CreateFormatConverter(converter.GetAddressOf())) ||
        FAILED(converter->Initialize(source.Get(), GUID_WICPixelFormat32bppPBGRA, WICBitmapDitherTypeNone, nullptr, 0.0,
                                     WICBitmapPaletteTypeMedianCut)))
    {
        return nullptr;
    }

    BitmapCacheEntry entry;
    entry.filePath = key;
    if (FAILED(target->CreateBitmapFromWicBitmap(converter.Get(), nullptr, entry.bitmap.GetAddressOf())))
    {
        return nullptr;
    }
    entry.size = entry.bitmap->GetSize();
    bitmapCache_.push_back(std::move(entry));
    if (size)
    {
        *size = bitmapCache_.back().size;
    }
    return bitmapCache_.back().bitmap.Get();
}
} // namespace msimeui
