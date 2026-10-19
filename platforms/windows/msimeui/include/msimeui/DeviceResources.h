#pragma once

#include "LruCache.h"

#include <d2d1.h>
#include <d2d1_1.h>
#include <d3d11.h>
#include <dcomp.h>
#include <dxgi1_2.h>
#include <dwrite.h>
#include <wincodec.h>
#include <memory>
#include <string>
#include <vector>
#include <wrl/client.h>

namespace msimeui
{
// SetBackdrop 那块圆角矩形里垫的系统材质，按 Fluent 亚克力与云母的配方叠三层：模糊的来源、把明暗拉到色调那边的亮度层（文字在亮背景或暗背景上都保得住对比度）、色调。
struct BackdropMaterial
{
    enum class Source
    {
        // 窗口后面的画面，模糊（亚克力）。
        WindowsBehind,
        // 桌面壁纸，模糊，不管中间隔着哪些窗口（云母）。要 Windows 11，没有时按 WindowsBehind 画。
        Wallpaper,
    };
    Source source = Source::WindowsBehind;
    // 只用 RGB；下面两个不透明度决定它露出多少。
    D2D1_COLOR_F tint = {0.0f, 0.0f, 0.0f, 1.0f};
    float tintOpacity = 0.0f;
    float luminosityOpacity = 0.0f;

    bool operator==(const BackdropMaterial &other) const
    {
        return source == other.source && tint.r == other.tint.r && tint.g == other.tint.g && tint.b == other.tint.b &&
               tintOpacity == other.tintOpacity && luminosityOpacity == other.luminosityOpacity;
    }
    bool operator!=(const BackdropMaterial &other) const
    {
        return !(*this == other);
    }
};

class DeviceResources
{
  public:
    DeviceResources();
    ~DeviceResources();
    DeviceResources(const DeviceResources &) = delete;
    DeviceResources &operator=(const DeviceResources &) = delete;

    bool EnsureFactories();
    bool EnsureForWindow(HWND hwnd);
    bool EnsureForComposition(HWND hwnd);
    void Resize(UINT width, UINT height);
    void DiscardTarget();
    HRESULT Present();
    // EndDraw 或 Present 的失败是不是设备丢了：EndDraw 报 D2DERR_RECREATE_TARGET，交换链的 Present 报 DXGI_ERROR_DEVICE_REMOVED 或 DXGI_ERROR_DEVICE_RESET（显卡驱动更新、TDR 重置显卡、远程会话换了适配器）。这几种只要 DiscardTarget，下一次绘制按新设备重建；其余失败（包括分层窗口的 UpdateLayeredWindow 失败）仍是真的画不出来。
    static bool IsDeviceLost(HRESULT hr);

    /// Rendering DPI override for windows whose system DPI diverges from the
    /// content's true scale (e.g. an RDP session syncs the client's display
    /// scaling into the session's DPI metadata while the focused application
    /// still renders at 96 DPI). Pass 0 (or any non-positive value) to clear
    /// the override and fall back to GetDpiForWindow(hwnd). While set, the
    /// override is the single DPI source so window sizing and rendering stay
    /// on the same scale; a live render target is re-DPI'd immediately
    /// because target recreation can be skipped for same-size compositions.
    void SetDpiOverride(FLOAT dpi);
    ID2D1RenderTarget *GetRenderTarget() const;
    // 窗口客户区按当前绘制 DPI 换算成的 DIP 尺寸。合成交换链按 64 像素分档、只增不减，渲染目标的 GetSize() 因而常比窗口大；内容按窗口排版的表面要用这个尺寸定外框，否则右边和下边会画到窗口外面被裁掉。还没有窗口时退回渲染目标的尺寸。
    D2D1_SIZE_F GetClientSize() const;
    ID2D1DeviceContext *GetDeviceContext() const;
    IDWriteFactory *GetDWriteFactory() const;
    ID2D1SolidColorBrush *GetSolidColorBrush(const D2D1_COLOR_F &color);
    IDWriteTextFormat *GetTextFormat(const std::wstring &fontFamily, float fontSize, DWRITE_FONT_WEIGHT fontWeight,
                                     DWRITE_TEXT_ALIGNMENT textAlignment, DWRITE_PARAGRAPH_ALIGNMENT paragraphAlignment,
                                     DWRITE_WORD_WRAPPING wordWrapping);
    ID2D1Bitmap *GetBitmapFromFile(const std::wstring &filePath, D2D1_SIZE_F *size = nullptr);
    // An icon already loaded from a module's resources. `key` names the cache
    // entry; the caller owns the HICON, which is only read here. Icons come
    // from the executable rather than a file so that a surface drawing its
    // product mark does not depend on the installer having copied an asset.
    ID2D1Bitmap *GetBitmapFromIcon(HICON icon, const std::wstring &key, D2D1_SIZE_F *size = nullptr);
    // UI thread only; drop file-backed images without discarding the device.
    void ClearBitmapCache() { bitmapCache_.clear(); }
    bool UsesComposition() const;

    // 系统材质模式：EnsureForComposition 改用 Windows.UI.Composition 的桌面窗口目标，GetRenderTarget() 画进一张离屏位图，Present() 把它拷进合成器的绘制表面，绘制表面下面垫一个按 SetBackdrop 裁成圆角矩形的系统模糊（HostBackdropBrush，或 Windows 11 的壁纸模糊）。内容和材质的形状在合成器的同一次提交里上屏，卡片伸缩时材质贴着它，不会像交换链 Present 与合成器提交那样各走各的、错开一帧。调用方的 BeginDraw/EndDraw/Present 不用改。改动在下一次 EnsureForComposition 生效；需要 Windows 10 1903，建不起来（或之后绘制表面出错）时 EnsureForComposition 返回 false，这时不走分层窗口回退，调用方应关掉材质模式再来一次。
    void SetBackdropComposition(bool enabled);
    bool UsesBackdropComposition() const;
    // 材质模式下这一帧材质的位置：rect 和 radius 是渲染目标坐标里的 DIP，opacity 是整块材质的不透明度（用户的候选窗不透明度、进出场动效）。在 Present 之前调用，和这一帧的内容同一次提交；空矩形把材质藏起来。不在材质模式时什么也不做。
    void SetBackdrop(const D2D1_RECT_F &rect, float radius, float opacity);
    // 材质参数，换了合成表面也保留。
    void SetBackdropMaterial(const BackdropMaterial &material);
    // 这台机器上建没建起壁纸模糊画刷（云母的来源）；没有时 Wallpaper 按 WindowsBehind 画。还没建过合成时是 false。
    bool BackdropWallpaperAvailable() const;
    // 材质模式最近一次失败在哪一步、HRESULT 是多少（例如 "CreateHostBackdropBrush 0x80004002"），没失败过时为空。只给诊断日志和截图工具看。
    std::string BackdropFailure() const;

  private:
    struct BrushCacheEntry
    {
        D2D1_COLOR_F color = {};
        Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> brush;
    };

    struct TextFormatKey
    {
        std::wstring fontFamily;
        float fontSize = 0.0f;
        DWRITE_FONT_WEIGHT fontWeight = DWRITE_FONT_WEIGHT_NORMAL;
        DWRITE_TEXT_ALIGNMENT textAlignment = DWRITE_TEXT_ALIGNMENT_LEADING;
        DWRITE_PARAGRAPH_ALIGNMENT paragraphAlignment = DWRITE_PARAGRAPH_ALIGNMENT_NEAR;
        DWRITE_WORD_WRAPPING wordWrapping = DWRITE_WORD_WRAPPING_WRAP;

        bool operator==(const TextFormatKey &other) const
        {
            return fontFamily == other.fontFamily && fontSize == other.fontSize && fontWeight == other.fontWeight &&
                   textAlignment == other.textAlignment && paragraphAlignment == other.paragraphAlignment &&
                   wordWrapping == other.wordWrapping;
        }
    };

    struct BitmapCacheEntry
    {
        std::wstring filePath;
        D2D1_SIZE_F size = {};
        Microsoft::WRL::ComPtr<ID2D1Bitmap> bitmap;
    };

    static bool IsSameColor(const D2D1_COLOR_F &lhs, const D2D1_COLOR_F &rhs);
    bool BindCompositionSurface();
    // Direct3D、DXGI 与 Direct2D 设备和设备上下文；交换链与系统材质两条合成路径共用。
    bool CreateRenderingDevice();
    bool EnsureBackdropSurface(HWND hwnd);
    bool BindBackdropTarget();
    bool ApplyBackdropMaterial();
    HRESULT PresentBackdrop();
    // 记下材质模式失败的那一步；成功时什么也不做。返回 hr 是否成功，好串成一条条件。
    bool BackdropStep(const char *step, HRESULT hr);
    // Windows.UI.Composition 的对象放在 DeviceResources.cpp 里，用 msimeui 的地方不必看到那些接口。
    struct BackdropComposition;
    std::unique_ptr<BackdropComposition> backdrop_;
    bool backdropComposition_ = false;
    // 绘制表面出过不是设备丢失的错：这次运行不再尝试材质模式。
    bool backdropBroken_ = false;
    bool backdropWallpaper_ = false;
    const char *backdropFailureStep_ = nullptr;
    HRESULT backdropFailureResult_ = S_OK;
    BackdropMaterial backdropMaterial_;
    FLOAT DpiForHwnd() const;

    FLOAT dpiOverride_ = 0.0f;

    HWND hwnd_ = nullptr;
    UINT pixelWidth_ = 1;
    UINT pixelHeight_ = 1;
    bool composition_ = false;
    // Drawn through a layered window when DirectComposition is unavailable.
    // Only reached where the composition path would have failed outright, so a
    // host that has a compositor never takes this route.
    bool EnsureCompositionSurface(HWND hwnd);
    // Created on demand: it is the only COM-based factory here, and a surface
    // that draws no bitmaps must not depend on the thread having initialised COM.
    bool EnsureImagingFactory();
    bool EnsureLayered(HWND hwnd, UINT width, UINT height);
    void DiscardLayered();
    bool layered_ = false;
    UINT layeredWidth_ = 0;
    UINT layeredHeight_ = 0;
    HDC layeredDC_ = nullptr;
    HBITMAP layeredBitmap_ = nullptr;
    HGDIOBJ layeredPrevious_ = nullptr;
    Microsoft::WRL::ComPtr<ID2D1DCRenderTarget> dcTarget_;

    Microsoft::WRL::ComPtr<ID2D1Factory> d2dFactory_;
    Microsoft::WRL::ComPtr<ID2D1Factory1> d2dFactory1_;
    Microsoft::WRL::ComPtr<IDWriteFactory> dwriteFactory_;
    Microsoft::WRL::ComPtr<IWICImagingFactory> wicFactory_;
    Microsoft::WRL::ComPtr<ID2D1HwndRenderTarget> hwndRenderTarget_;
    Microsoft::WRL::ComPtr<ID3D11Device> d3dDevice_;
    Microsoft::WRL::ComPtr<IDXGIDevice> dxgiDevice_;
    Microsoft::WRL::ComPtr<IDXGISwapChain1> swapChain_;
    Microsoft::WRL::ComPtr<ID2D1Device> d2dDevice_;
    Microsoft::WRL::ComPtr<ID2D1DeviceContext> deviceContext_;
    Microsoft::WRL::ComPtr<ID2D1Bitmap1> dxgiBitmap_;
    Microsoft::WRL::ComPtr<IDCompositionDevice> dcompDevice_;
    Microsoft::WRL::ComPtr<IDCompositionTarget> dcompTarget_;
    Microsoft::WRL::ComPtr<IDCompositionVisual> dcompVisual_;
    std::vector<BrushCacheEntry> brushCache_;
    LruCache<TextFormatKey, Microsoft::WRL::ComPtr<IDWriteTextFormat>> textFormatCache_{
        kTextFormatCacheCapacity};
    std::vector<BitmapCacheEntry> bitmapCache_;
};
} // namespace msimeui
