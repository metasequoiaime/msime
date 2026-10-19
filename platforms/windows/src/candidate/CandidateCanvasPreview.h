#pragma once
#include "CandidateCanvasDrawing.h"
#include "CandidatePalette.h"
#include <array>
#include <dwrite.h>
#include <map>
#include <string_view>
#include <wincodec.h>

namespace msime::windows {
// 设置卡片与社区导出的静态样例共用此绘制入口；只接收已安全读取的图片，不能自行打开路径。
inline Microsoft::WRL::ComPtr<IWICBitmap> candidate_canvas_preview(
    IWICImagingFactory *wic, CandidateSkinCanvas canvas,
    CandidatePalette palette, CandidatePalette panel_palette, bool dark,
    bool vertical, double min_width,
    const std::function<
        Microsoft::WRL::ComPtr<IWICBitmap>(const std::wstring &)> &decode) {
  using Microsoft::WRL::ComPtr;
  std::map<std::wstring, ComPtr<IWICBitmap>> images;
  canvas = candidate_canvas_drawable(std::move(canvas), !dark,
                                     [&](const std::wstring &key) {
                                       if (images.find(key) != images.end())
                                         return true;
                                       auto image = decode(key);
                                       if (!image)
                                         return false;
                                       images.emplace(key, std::move(image));
                                       return true;
                                     });
  if (!canvas.active)
    return nullptr;
  if (candidate_canvas_panel_drawn(canvas))
    palette = panel_palette;
  ComPtr<ID2D1Factory> d2d;
  ComPtr<IDWriteFactory> dwrite;
  if (FAILED(D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED,
                               d2d.GetAddressOf())) ||
      FAILED(DWriteCreateFactory(
          DWRITE_FACTORY_TYPE_SHARED, __uuidof(IDWriteFactory),
          reinterpret_cast<IUnknown **>(dwrite.GetAddressOf()))))
    return nullptr;
  auto format = [&](float size) {
    ComPtr<IDWriteTextFormat> value;
    dwrite->CreateTextFormat(L"Segoe UI", nullptr, DWRITE_FONT_WEIGHT_NORMAL,
                             DWRITE_FONT_STYLE_NORMAL,
                             DWRITE_FONT_STRETCH_NORMAL, size, L"zh-CN",
                             value.GetAddressOf());
    if (value) {
      value->SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP);
      value->SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER);
    }
    return value;
  };
  auto label_format = format(12), word_format = format(16),
       preedit_format = format(13);
  if (!label_format || !word_format || !preedit_format)
    return nullptr;
  auto layout = [&](std::wstring_view text, IDWriteTextFormat *font) {
    ComPtr<IDWriteTextLayout> value;
    dwrite->CreateTextLayout(text.data(), static_cast<UINT32>(text.size()),
                             font, 4096, 28, value.GetAddressOf());
    return value;
  };
  auto advance = [](IDWriteTextLayout *text) {
    DWRITE_TEXT_METRICS metrics{};
    if (text)
      text->GetMetrics(&metrics);
    return metrics.widthIncludingTrailingWhitespace;
  };
  constexpr std::array<std::wstring_view, 5> words{L"你们", L"你", L"尼", L"妮",
                                                   L"泥"};
  std::array<ComPtr<IDWriteTextLayout>, 5> labels, texts;
  std::array<double, 5> cells{};
  double list_width = 0;
  for (size_t i = 0; i < words.size(); ++i) {
    labels[i] = layout(std::to_wstring(i + 1), label_format.Get());
    texts[i] = layout(words[i], word_format.Get());
    if (!labels[i] || !texts[i])
      return nullptr;
    cells[i] = advance(labels[i].Get()) + advance(texts[i].Get()) + 20;
    list_width = vertical ? (std::max)(list_width, cells[i])
                          : list_width + cells[i] + (i ? 4 : 0);
  }
  auto preedit = layout(L"ni'mf", preedit_format.Get());
  if (!preedit)
    return nullptr;
  const CandidateCanvasEdges base{6, 8, 6, 8};
  const auto extra = candidate_canvas_content_offset(canvas, base);
  const CandidateCanvasEdges content{
      base.top + extra.top, base.right + extra.right,
      base.bottom + extra.bottom, base.left + extra.left};
  const double gap = canvas.line_gap.value_or(4);
  const double panel_width =
      (std::max)({min_width, content.left + content.right + list_width,
                  content.left + content.right + advance(preedit.Get())});
  const double panel_height = content.top + 28 + gap +
                              (vertical ? 5 * 28 + 4 * 4 : 28) + content.bottom;
  const auto margins = candidate_canvas_margins(canvas, {16, 16, 16, 16},
                                                panel_width, panel_height);
  const CandidateCanvasRect panel{margins.left, margins.top,
                                  margins.left + panel_width,
                                  margins.top + panel_height};
  constexpr double scale = 2;
  const double width = std::ceil((panel.right + margins.right) * scale);
  const double height = std::ceil((panel.bottom + margins.bottom) * scale);
  // catalog 的几何有各项上限，组合后仍要限制整张位图的分配。
  if (!(width > 0 && height > 0) || width > 8192 || height > 8192 ||
      width * height > 16 * 1024 * 1024)
    return nullptr;
  ComPtr<IWICBitmap> bitmap;
  ComPtr<ID2D1RenderTarget> target;
  const auto properties = D2D1::RenderTargetProperties(
      D2D1_RENDER_TARGET_TYPE_DEFAULT,
      D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,
                        D2D1_ALPHA_MODE_PREMULTIPLIED),
      96, 96);
  if (FAILED(wic->CreateBitmap(static_cast<UINT>(width),
                               static_cast<UINT>(height),
                               GUID_WICPixelFormat32bppPBGRA,
                               WICBitmapCacheOnLoad, bitmap.GetAddressOf())) ||
      FAILED(d2d->CreateWicBitmapRenderTarget(bitmap.Get(), properties,
                                              target.GetAddressOf())))
    return nullptr;
  std::map<std::wstring, ComPtr<ID2D1Bitmap>> sprites;
  for (const auto &[key, image] : images) {
    ComPtr<ID2D1Bitmap> sprite;
    // 源文件 DPI 不能改变清单中以源像素表示的九宫格切片。
    const auto bitmap_properties =
        D2D1::BitmapProperties(D2D1::PixelFormat(DXGI_FORMAT_B8G8R8A8_UNORM,
                                                 D2D1_ALPHA_MODE_PREMULTIPLIED),
                               96, 96);
    if (FAILED(target->CreateBitmapFromWicBitmap(
            image.Get(), &bitmap_properties, sprite.GetAddressOf())))
      return nullptr;
    sprites.emplace(key, std::move(sprite));
  }
  auto rectangle = [](const CandidateCanvasRect &r) {
    return D2D1::RectF(static_cast<float>(r.left), static_cast<float>(r.top),
                       static_cast<float>(r.right),
                       static_cast<float>(r.bottom));
  };
  auto solid = [&](CandidateColor color) {
    ComPtr<ID2D1SolidColorBrush> brush;
    target->CreateSolidColorBrush(
        D2D1::ColorF(color.r, color.g, color.b, color.a), brush.GetAddressOf());
    return brush;
  };
  auto decorations = [&](const std::vector<CandidateCanvasDecoration> &layer) {
    for (const auto &decoration : layer) {
      const auto &sprite = sprites.at(decoration.image);
      const auto size = sprite->GetSize();
      if (const auto r = candidate_canvas_contain(
              candidate_canvas_decoration_box(decoration, panel), size.width,
              size.height))
        target->DrawBitmap(sprite.Get(), rectangle(*r));
    }
  };
  auto text_at = [&](IDWriteTextLayout *text, double x, double y,
                     CandidateColor color) {
    if (auto brush = solid(color))
      target->DrawTextLayout(
          D2D1::Point2F(static_cast<float>(x), static_cast<float>(y)), text,
          brush.Get());
  };
  const auto layers = candidate_canvas_layers(canvas.decorations);
  target->BeginDraw();
  target->SetTransform(D2D1::Matrix3x2F::Scale(static_cast<float>(scale),
                                               static_cast<float>(scale)));
  target->Clear(dark ? D2D1::ColorF(0x16181D) : D2D1::ColorF(0xE9EDF3));
  decorations(layers.below);
  if (candidate_canvas_panel_drawn(canvas))
    draw_canvas_panel(target.Get(), sprites.at(canvas.border->source).Get(),
                      *canvas.border, panel, 1);
  else {
    const auto card =
        D2D1::RoundedRect(rectangle(panel), palette.radius, palette.radius);
    if (auto brush = solid(palette.surface))
      target->FillRoundedRectangle(card, brush.Get());
    if (auto brush = solid(palette.border); brush && palette.border_width > 0)
      target->DrawRoundedRectangle(card, brush.Get(), palette.border_width);
  }
  const double left = panel.left + content.left;
  const double preedit_top = panel.top + content.top;
  const auto preedit_rect = D2D1::RectF(static_cast<float>(left),
                                       static_cast<float>(preedit_top),
                                       static_cast<float>(panel.right - content.right),
                                       static_cast<float>(preedit_top + 28));
  if (palette.preedit_background)
    if (auto brush = solid(*palette.preedit_background))
      target->FillRoundedRectangle(
          D2D1::RoundedRect(preedit_rect, palette.item_radius, palette.item_radius), brush.Get());
  if (palette.preedit_divider)
    if (auto brush = solid(*palette.preedit_divider))
      target->FillRectangle(D2D1::RectF(preedit_rect.left, preedit_rect.bottom - 1,
                                        preedit_rect.right, preedit_rect.bottom), brush.Get());
  text_at(preedit.Get(), left, panel.top + content.top,
          palette.preedit_text.value_or(palette.accent));
  double x = left, y = panel.top + content.top + 28 + gap;
  for (size_t i = 0; i < words.size(); ++i) {
    const double cell =
        vertical ? panel_width - content.left - content.right : cells[i];
    if (i == 0)
      if (auto brush = solid(palette.selected))
        target->FillRoundedRectangle(
            D2D1::RoundedRect(D2D1::RectF(static_cast<float>(x),
                                          static_cast<float>(y),
                                          static_cast<float>(x + cell),
                                          static_cast<float>(y + 28)),
                              palette.item_radius, palette.item_radius),
            brush.Get());
    text_at(labels[i].Get(), x + 8, y,
            candidate_row_number_color(palette, i == 0));
    text_at(texts[i].Get(), x + 12 + advance(labels[i].Get()), y,
            candidate_row_text_color(palette, palette.candidate_text.value_or(palette.text),
                                      i == 0, false));
    if (vertical)
      y += 32;
    else
      x += cell + 4;
  }
  decorations(layers.above);
  if (FAILED(target->EndDraw()))
    return nullptr;
  return bitmap;
}
} // namespace msime::windows
