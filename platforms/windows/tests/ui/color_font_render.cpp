#include "../../common/Windows7EmojiFont.h"
#include <cstdio>
#include <stdexcept>

using Microsoft::WRL::ComPtr;
namespace {
void require(HRESULT result) {
  if (FAILED(result)) {
    std::fprintf(stderr, "Direct2D render failed: 0x%08lx\n", static_cast<unsigned long>(result));
    throw std::runtime_error("Text drawing failed");
  }
}
// 独立的内存 DC 不依赖交互桌面、私有字体文件或 Rust 库，可直接带到 Win7 运行。
struct Surface {
  HDC dc = CreateCompatibleDC(nullptr);
  HBITMAP bitmap = nullptr;
  HGDIOBJ previous = nullptr;
  void *pixels = nullptr;
  Surface() {
    BITMAPINFO info{};
    info.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    info.bmiHeader.biWidth = 200;
    info.bmiHeader.biHeight = -100;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &pixels, nullptr, 0);
    if (!dc || !bitmap || !pixels)
      throw std::runtime_error("Memory bitmap unavailable");
    previous = SelectObject(dc, bitmap);
  }
  ~Surface() {
    SelectObject(dc, previous);
    DeleteObject(bitmap);
    DeleteDC(dc);
  }
};
} // 匿名命名空间

int main() {
  using namespace msime::windows::compat;
  try {
    ComPtr<ID2D1Factory> factory;
    require(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, factory.GetAddressOf()));
    ComPtr<IDWriteFactory> text;
    require(DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
                               reinterpret_cast<IUnknown **>(text.GetAddressOf())));
    ComPtr<ID2D1DCRenderTarget> target;
    const auto properties = D2D1::RenderTargetProperties(
        D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM, D2D1_ALPHA_MODE_IGNORE));
    require(factory->CreateDCRenderTarget(&properties, &target));
    Surface surface;
    const RECT rectangle{0, 0, 200, 100};
    require(target->BindDC(surface.dc, &rectangle));
    ComPtr<IDWriteTextFormat> format;
    require(text->CreateTextFormat(L"Arial", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
                                  DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL,
                                  24, L"en-US", &format));
    ComPtr<ID2D1SolidColorBrush> brush;
    require(target->CreateSolidColorBrush(D2D1::ColorF(0.f, 0.f, 0.f), &brush));
    constexpr wchar_t sample[] = L"Synthetic";
    ComPtr<IDWriteTextLayout> layout;
    require(CreateTextLayoutWithWindows7Emoji(text.Get(), sample, 9, format.Get(), 200, 100, &layout));
    for (const auto options : {D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
                              static_cast<D2D1_DRAW_TEXT_OPTIONS>(D2D1_DRAW_TEXT_OPTIONS_CLIP | D2D1_DRAW_TEXT_OPTIONS_NO_SNAP)}) {
      for (const bool styled : {false, true}) {
        target->BeginDraw();
        target->Clear(D2D1::ColorF(1.f, 1.f, 1.f));
        if (styled)
          target->DrawTextLayout(D2D1::Point2F(0, 0), layout.Get(), brush.Get(), enable_color_font(options));
        else
          DrawTextWithWindows7Emoji(text.Get(), target.Get(), sample, 9, format.Get(),
                                   D2D1::RectF(0, 0, 200, 100), brush.Get(), options);
        require(target->EndDraw());
        GdiFlush();
        const auto *pixels = static_cast<const unsigned char *>(surface.pixels);
        size_t ink = 0;
        for (size_t offset = 0; offset < 200 * 100 * 4; offset += 4)
          if (pixels[offset] < 128 && pixels[offset + 1] < 128 && pixels[offset + 2] < 128)
            ++ink;
        if (ink < 50)
          throw std::runtime_error("Text drawing produced a blank bitmap");
      }
    }
    std::puts("Direct2D text and styled layout: six draws completed with visible glyphs");
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "%s\n", error.what());
    return 1;
  }
}
