#pragma once
#include "CandidateCanvas.h"
#include <d2d1_1.h>
#include <wrl/client.h>

namespace msime::windows {
// canvas 皮肤的九宫格面板画进 panel（candidate_nine_slice），块的边按当前变换和
// DPI 对齐到设备像素。照 msimeui 的
// DrawNineSlice，每块用夹住边缘的图像画刷来画：DrawBitmap
// 缩放时会采到源矩形外面，把相邻那块的颜色渗进接缝。拿不到
// ID2D1DeviceContext（分层窗口的回退）时退回 DrawBitmap。
inline void draw_canvas_panel(ID2D1RenderTarget *target, ID2D1Bitmap *bitmap,
                              const CandidateCanvasBorder &border,
                              const CandidateCanvasRect &panel, float opacity) {
  if (!(opacity > 0.0f))
    return;
  const D2D1_SIZE_U pixels = bitmap->GetPixelSize();
  const D2D1_SIZE_F dips = bitmap->GetSize();
  float dpi_x = 96.0f, dpi_y = 96.0f;
  target->GetDpi(&dpi_x, &dpi_y);
  D2D1_MATRIX_3X2_F transform{};
  target->GetTransform(&transform);
  CandidateCanvasSnap snap;
  snap.enabled = transform._12 == 0.0f && transform._21 == 0.0f &&
                 transform._11 > 0.0f && transform._22 > 0.0f && dpi_x > 0.0f &&
                 dpi_y > 0.0f;
  snap.scale_x = transform._11 * dpi_x / 96.0;
  snap.scale_y = transform._22 * dpi_y / 96.0;
  snap.offset_x = transform._31 * dpi_x / 96.0;
  snap.offset_y = transform._32 * dpi_y / 96.0;
  Microsoft::WRL::ComPtr<ID2D1DeviceContext> context;
  target->QueryInterface(IID_PPV_ARGS(context.GetAddressOf()));
  for (const auto &tile : candidate_nine_slice(
           pixels.width, pixels.height, dips.width, dips.height, panel,
           border.slice, border.width, border.fill, snap)) {
    const D2D1_RECT_F source{static_cast<float>(tile.source.left),
                             static_cast<float>(tile.source.top),
                             static_cast<float>(tile.source.right),
                             static_cast<float>(tile.source.bottom)};
    const D2D1_RECT_F part{static_cast<float>(tile.destination.left),
                           static_cast<float>(tile.destination.top),
                           static_cast<float>(tile.destination.right),
                           static_cast<float>(tile.destination.bottom)};
    Microsoft::WRL::ComPtr<ID2D1ImageBrush> image;
    if (context) {
      // 图像画刷把源矩形的左上角放在画刷原点上，所以先缩放到这一块的大小，再挪到这一块的位置。
      const auto placement =
          D2D1::Matrix3x2F::Scale(
              (part.right - part.left) / (source.right - source.left),
              (part.bottom - part.top) / (source.bottom - source.top)) *
          D2D1::Matrix3x2F::Translation(part.left, part.top);
      if (FAILED(context->CreateImageBrush(
              bitmap,
              D2D1::ImageBrushProperties(source, D2D1_EXTEND_MODE_CLAMP,
                                         D2D1_EXTEND_MODE_CLAMP,
                                         D2D1_INTERPOLATION_MODE_LINEAR),
              D2D1::BrushProperties(opacity, placement), image.GetAddressOf())))
        image.Reset();
    }
    if (image)
      context->FillRectangle(part, image.Get());
    else
      target->DrawBitmap(bitmap, part, opacity,
                         D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, &source);
  }
}
} // namespace msime::windows
