#pragma once

#include "LruCache.h"

#include <d2d1.h>
#include <d2d1_1.h>
#include <d3d11.h>
#include <dcomp.h>
#include <dxgi1_2.h>
#include <dwrite.h>
#include <wincodec.h>
#include <string>
#include <vector>
#include <wrl/client.h>

namespace msimeui
{
class DeviceResources
{
  public:
    bool EnsureFactories();
    bool EnsureForWindow(HWND hwnd);
    bool EnsureForComposition(HWND hwnd);
    void Resize(UINT width, UINT height);
    void DiscardTarget();
    HRESULT Present();

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
