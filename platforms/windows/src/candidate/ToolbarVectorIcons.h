#pragma once
#include "ToolbarIcons.h"
#include <d2d1.h>
#include <wrl/client.h>
#include <cmath>
#include <initializer_list>
#include <stdexcept>

namespace msime::windows {
// 图标字体不可用时使用同一套 24 DIP 线性图标，不依赖 Win7 缺少的字库。
inline bool draw_toolbar_vector_icon(ID2D1RenderTarget *target, int button,
                                     const D2D1_RECT_F &cell, float size,
                                     ID2D1Brush *brush) {
  switch (button) {
  case kToolbarInputScheme:
  case kToolbarEmoji:
  case kToolbarHandwriting:
  case kToolbarScreenKeyboard:
  case kToolbarVoice:
  case kToolbarSettings:
    break;
  default:
    return false;
  }
  Microsoft::WRL::ComPtr<ID2D1Factory> factory;
  target->GetFactory(factory.GetAddressOf());
  Microsoft::WRL::ComPtr<ID2D1StrokeStyle> stroke;
  auto style = D2D1::StrokeStyleProperties();
  style.startCap = style.endCap = style.dashCap = D2D1_CAP_STYLE_ROUND;
  style.lineJoin = D2D1_LINE_JOIN_ROUND;
  if (FAILED(factory->CreateStrokeStyle(style, nullptr, 0, stroke.GetAddressOf())))
    throw std::runtime_error("Toolbar icon stroke unavailable");
  struct RestoreTransform {
    ID2D1RenderTarget *target;
    D2D1_MATRIX_3X2_F previous;
    ~RestoreTransform() { target->SetTransform(previous); }
  } restore{target, {}};
  target->GetTransform(&restore.previous);
  target->SetTransform(
      D2D1::Matrix3x2F::Scale(size / 24.0f, size / 24.0f) *
      D2D1::Matrix3x2F::Translation((cell.left + cell.right - size) / 2.0f,
                                   (cell.top + cell.bottom - size) / 2.0f) *
      restore.previous);
  constexpr float width = 1.7f;
  const auto line = [&](float x1, float y1, float x2, float y2) {
    target->DrawLine({x1, y1}, {x2, y2}, brush, width, stroke.Get());
  };
  const auto path = [&](std::initializer_list<D2D1_POINT_2F> points, bool closed) {
    Microsoft::WRL::ComPtr<ID2D1PathGeometry> geometry;
    Microsoft::WRL::ComPtr<ID2D1GeometrySink> sink;
    if (FAILED(factory->CreatePathGeometry(geometry.GetAddressOf())) ||
        FAILED(geometry->Open(sink.GetAddressOf())))
      throw std::runtime_error("Toolbar icon path unavailable");
    auto point = points.begin();
    sink->BeginFigure(*point++, D2D1_FIGURE_BEGIN_HOLLOW);
    while (point != points.end()) sink->AddLine(*point++);
    sink->EndFigure(closed ? D2D1_FIGURE_END_CLOSED : D2D1_FIGURE_END_OPEN);
    if (FAILED(sink->Close()))
      throw std::runtime_error("Toolbar icon path incomplete");
    target->DrawGeometry(geometry.Get(), brush, width, stroke.Get());
  };
  const auto curve = [&](D2D1_POINT_2F start, D2D1_POINT_2F a,
                          D2D1_POINT_2F b, D2D1_POINT_2F end) {
    Microsoft::WRL::ComPtr<ID2D1PathGeometry> geometry;
    Microsoft::WRL::ComPtr<ID2D1GeometrySink> sink;
    if (FAILED(factory->CreatePathGeometry(geometry.GetAddressOf())) ||
        FAILED(geometry->Open(sink.GetAddressOf())))
      throw std::runtime_error("Toolbar icon curve unavailable");
    sink->BeginFigure(start, D2D1_FIGURE_BEGIN_HOLLOW);
    sink->AddBezier({a, b, end});
    sink->EndFigure(D2D1_FIGURE_END_OPEN);
    if (FAILED(sink->Close()))
      throw std::runtime_error("Toolbar icon curve incomplete");
    target->DrawGeometry(geometry.Get(), brush, width, stroke.Get());
  };
  switch (button) {
  case kToolbarInputScheme:
    for (float y : {6.0f, 12.0f, 18.0f}) {
      target->FillEllipse(D2D1::Ellipse({3.5f, y}, 1.0f, 1.0f), brush);
      line(8.0f, y, 21.0f, y);
    }
    break;
  case kToolbarEmoji:
    target->DrawEllipse(D2D1::Ellipse({12, 12}, 9, 9), brush, width, stroke.Get());
    for (float x : {8.5f, 15.5f})
      target->FillEllipse(D2D1::Ellipse({x, 9.5f}, 0.9f, 0.9f), brush);
    curve({8, 14}, {10, 17}, {14, 17}, {16, 14});
    break;
  case kToolbarHandwriting:
    path({{4, 20}, {5.2f, 15}, {16.5f, 3.7f}, {20.3f, 7.5f}, {9, 18.8f}}, true);
    line(5.2f, 15, 9, 18.8f);
    line(14.4f, 5.8f, 18.2f, 9.6f);
    break;
  case kToolbarScreenKeyboard:
    target->DrawRoundedRectangle(D2D1::RoundedRect({2.5f, 5, 21.5f, 19}, 2, 2),
                                 brush, width, stroke.Get());
    for (float y : {9.0f, 12.0f})
      for (float x : {6.0f, 10.0f, 14.0f, 18.0f})
        target->FillRoundedRectangle(
            D2D1::RoundedRect({x - 0.7f, y - 0.6f, x + 0.7f, y + 0.6f}, 0.3f, 0.3f), brush);
    line(8, 16, 16, 16);
    break;
  case kToolbarVoice:
    target->DrawRoundedRectangle(D2D1::RoundedRect({9, 2.5f, 15, 14.5f}, 3, 3),
                                 brush, width, stroke.Get());
    curve({5.5f, 11.5f}, {5.5f, 20.5f}, {18.5f, 20.5f}, {18.5f, 11.5f});
    line(12, 18.5f, 12, 22);
    line(8, 22, 16, 22);
    break;
  case kToolbarSettings: {
    // 八齿轮廓与圆形轴孔，避免将设置退化为「设」字。
    Microsoft::WRL::ComPtr<ID2D1PathGeometry> geometry;
    Microsoft::WRL::ComPtr<ID2D1GeometrySink> sink;
    if (FAILED(factory->CreatePathGeometry(geometry.GetAddressOf())) ||
        FAILED(geometry->Open(sink.GetAddressOf())))
      throw std::runtime_error("Toolbar settings icon unavailable");
    constexpr float pi = 3.14159265358979323846f;
    for (int point = 0; point < 48; ++point) {
      const float angle = static_cast<float>(point) * pi / 24.0f;
      const float radius = point % 6 >= 2 && point % 6 <= 3 ? 10.0f : 8.0f;
      const D2D1_POINT_2F vertex{12 + radius * std::cos(angle), 12 + radius * std::sin(angle)};
      if (!point) sink->BeginFigure(vertex, D2D1_FIGURE_BEGIN_HOLLOW);
      else sink->AddLine(vertex);
    }
    sink->EndFigure(D2D1_FIGURE_END_CLOSED);
    if (FAILED(sink->Close()))
      throw std::runtime_error("Toolbar settings icon incomplete");
    target->DrawGeometry(geometry.Get(), brush, width, stroke.Get());
    target->DrawEllipse(D2D1::Ellipse({12, 12}, 3, 3), brush, width, stroke.Get());
    break;
  }
  }
  return true;
}
} // namespace msime::windows
