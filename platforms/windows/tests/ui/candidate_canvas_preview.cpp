#include "candidate/CandidateCanvasPreview.h"
#include <iostream>
#include <stdexcept>
#include <windows.h>

using namespace msime::windows;
using Microsoft::WRL::ComPtr;
namespace {
void check(bool condition, const char *message) {
  if (!condition)
    throw std::runtime_error(message);
}
ComPtr<IWICBitmap> image(IWICImagingFactory *wic,
                         const std::array<uint32_t, 9> &colors) {
  std::array<uint32_t, 36> pixels{};
  for (size_t y = 0; y < 6; ++y)
    for (size_t x = 0; x < 6; ++x)
      pixels[y * 6 + x] = 0xff000000 | colors[(y / 2) * 3 + x / 2];
  ComPtr<IWICBitmap> bitmap;
  check(SUCCEEDED(wic->CreateBitmapFromMemory(
            6, 6, GUID_WICPixelFormat32bppPBGRA, 24,
            static_cast<UINT>(sizeof(pixels)),
            reinterpret_cast<BYTE *>(pixels.data()), bitmap.GetAddressOf())),
        "fixture bitmap");
  return bitmap;
}
struct Pixels {
  UINT width = 0, height = 0;
  std::vector<uint32_t> values;
  explicit Pixels(IWICBitmap *bitmap) {
    check(bitmap != nullptr, "preview bitmap");
    check(SUCCEEDED(bitmap->GetSize(&width, &height)), "preview dimensions");
    values.resize(static_cast<size_t>(width) * height);
    check(SUCCEEDED(bitmap->CopyPixels(
              nullptr, width * 4, static_cast<UINT>(values.size() * 4),
              reinterpret_cast<BYTE *>(values.data()))),
          "preview pixels");
  }
  size_t count(uint32_t rgb) const {
    return static_cast<size_t>(
        std::count(values.begin(), values.end(), 0xff000000 | rgb));
  }
};
} // namespace
int main() {
  const auto apartment = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
  if (FAILED(apartment))
    return 1;
  int result = 0;
  try {
    ComPtr<IWICImagingFactory> wic;
    check(SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr,
                                     CLSCTX_INPROC_SERVER,
                                     IID_PPV_ARGS(wic.GetAddressOf()))),
          "WIC factory");
    constexpr std::array<uint32_t, 9> colors{0xff0000, 0x00ff00, 0x0000ff,
                                             0xffff00, 0x00ffff, 0xff00ff,
                                             0x804000, 0x408000, 0x400080};
    const auto panel = image(wic.Get(), colors);
    const auto dark =
        image(wic.Get(), {0x004080, 0x004080, 0x004080, 0x004080, 0x004080,
                          0x004080, 0x004080, 0x004080, 0x004080});
    const auto front =
        image(wic.Get(), {0x102030, 0x102030, 0x102030, 0x102030, 0x102030,
                          0x102030, 0x102030, 0x102030, 0x102030});
    CandidateSkinCanvas canvas;
    canvas.active = true;
    canvas.border = CandidateCanvasBorder{
        L"dark", L"light", {2, 2, 2, 2}, {4, 4, 4, 4}, true};
    canvas.content_insets = CandidateCanvasEdges{8, 8, 8, 8};
    canvas.outer_insets = CandidateCanvasEdges{0, 0, 0, 0};
    CandidatePalette fallback = candidate_native_palette(false),
                     drawn = fallback;
    fallback.surface = candidate_rgb(0x765432);
    fallback.text = candidate_rgb(0x234567);
    drawn.text = candidate_rgb(0x654321);
    drawn.preedit_text = candidate_rgb(0x443322);
    drawn.preedit_background = candidate_rgb(0x112244);
    drawn.preedit_divider = candidate_rgb(0x556688);
    auto decode = [&](const std::wstring &key) -> ComPtr<IWICBitmap> {
      if (key == L"light")
        return panel;
      if (key == L"dark")
        return dark;
      if (key == L"front")
        return front;
      return nullptr;
    };
    UINT horizontal_height = 0;
    for (const bool vertical : {false, true}) {
      const auto bitmap = candidate_canvas_preview(
          wic.Get(), canvas, fallback, drawn, false, vertical, 0, decode);
      const Pixels light(bitmap.Get());
      for (const auto color : colors) {
        if (light.count(color) <= 16)
          std::cerr << "missing region " << std::hex << color << std::dec
                    << ": " << light.count(color) << " pixels\n";
        check(light.count(color) > 16,
              "all nine panel regions must be visible");
      }
      check(light.count(0x112244) > 100 && light.count(0x556688) > 100,
            "preedit band and divider must use the panel palette");
      if (!vertical)
        horizontal_height = light.height;
      else
        check(light.height > horizontal_height, "vertical samples must stack");
      const Pixels night(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                  drawn, true, vertical, 0,
                                                  decode)
                             .Get());
      check(night.count(0x004080) > 100, "dark slot must use its source");
      check(night.count(0xff0000) == 0,
            "dark slot must not retain light pixels");
    }
    const Pixels original(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                   drawn, false, false, 0,
                                                   decode)
                              .Get());
    const Pixels wide(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                               drawn, false, false, 1000,
                                               decode)
                          .Get());
    check(wide.width == 2064, "minimum width must apply to the whole panel");
    canvas.content_insets = CandidateCanvasEdges{10, 12, 14, 16};
    const Pixels content(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                  drawn, false, false, 0,
                                                  decode)
                             .Get());
    check(content.width == original.width + 24 &&
              content.height == original.height + 16,
          "asymmetric content insets must affect the actual panel");
    canvas.content_insets = CandidateCanvasEdges{8, 8, 8, 8};
    canvas.line_gap = 17;
    const Pixels spaced(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                 drawn, false, false, 0, decode)
                            .Get());
    check(spaced.height == original.height + 26,
          "line gap must change actual raster height");
    canvas.outer_insets = CandidateCanvasEdges{10, 12, 14, 16};
    const Pixels inset(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                drawn, false, false, 0, decode)
                           .Get());
    check(inset.width == spaced.width + 56 &&
              inset.height == spaced.height + 48,
          "outer insets must survive rasterization");
    canvas.outer_insets = CandidateCanvasEdges{};
    CandidateCanvasDecoration decoration;
    decoration.image = L"front";
    decoration.anchor = CandidateCanvasAnchor::top_left;
    decoration.width = decoration.height = 8;
    decoration.z_index = -1;
    canvas.decorations = {decoration};
    const Pixels below(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                drawn, false, false, 0, decode)
                           .Get());
    check(below.count(0x102030) == 0, "panel must cover lower decoration");
    canvas.decorations[0].z_index = 2;
    const Pixels above(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                drawn, false, false, 0, decode)
                           .Get());
    check(above.count(0x102030) > 100, "upper decoration must cover panel");
    canvas.decorations.clear();
    canvas.border->light_source = L"broken";
    const Pixels broken(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                 drawn, false, false, 0, decode)
                            .Get());
    check(broken.count(0x765432) > 100, "bad image must use safe surface");
    check(broken.count(0x654321) == 0,
          "bad image must not use panel text color");
    check(broken.count(0x112244) == 0 && broken.count(0x556688) == 0,
          "bad image must not retain the panel preedit band");
    canvas.outer_insets = CandidateCanvasEdges{1e9, 1e9, 1e9, 1e9};
    const Pixels bounded(candidate_canvas_preview(wic.Get(), canvas, fallback,
                                                  drawn, false, false, 0,
                                                  decode)
                             .Get());
    check(bounded.width < 8192 && bounded.height < 8192,
          "outer reach must remain bounded");
    check(!candidate_canvas_preview(wic.Get(), canvas, fallback, drawn, false,
                                    false, 1e9, decode),
          "oversized allocation must fail");
    std::cout << "canvas preview: nine regions, modes, layouts, insets, gap, "
                 "layers, fallback and bounds passed\n";
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    result = 1;
  }
  CoUninitialize();
  return result;
}
