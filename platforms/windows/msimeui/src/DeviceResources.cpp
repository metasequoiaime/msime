#include "msimeui/DeviceResources.h"

#include "msimeui/Fonts.h"

#include <algorithm>
#include <windows.h>

namespace msimeui
{
namespace
{
UINT AlignSwapExtent(UINT value)
{
    constexpr UINT kBucket = 64;
    value = (std::max)(value, 1U);
    return ((value + kBucket - 1U) / kBucket) * kBucket;
}
} // namespace

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
    const UINT dpi = GetDpiForWindow(hwnd_);
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

        UINT flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;
        D3D_FEATURE_LEVEL featureLevel = D3D_FEATURE_LEVEL_11_0;
        Microsoft::WRL::ComPtr<ID3D11DeviceContext> ignored;
        if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION,
                                     d3dDevice_.GetAddressOf(), &featureLevel, ignored.GetAddressOf())))
        {
            if (FAILED(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_WARP, nullptr, flags, nullptr, 0, D3D11_SDK_VERSION,
                                         d3dDevice_.GetAddressOf(), &featureLevel, ignored.GetAddressOf())))
            {
                composition_ = false;
                return false;
            }
        }
        if (FAILED(d3dDevice_.As(&dxgiDevice_)))
        {
            composition_ = false;
            return false;
        }
        if (FAILED(d2dFactory1_->CreateDevice(dxgiDevice_.Get(), d2dDevice_.GetAddressOf())) ||
            FAILED(d2dDevice_->CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE, deviceContext_.GetAddressOf())))
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

        if (FAILED(DCompositionCreateDevice(dxgiDevice_.Get(), IID_PPV_ARGS(dcompDevice_.GetAddressOf()))) ||
            FAILED(dcompDevice_->CreateTargetForHwnd(hwnd, TRUE, dcompTarget_.GetAddressOf())) ||
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
