#pragma once

#include "SettingsWindow.h"
#include "ThemeData.h"
#include "ControlText.h"
#include "../PunctuationSettings.h"
#include "../../src/candidate/FloatingToolbarSettings.h"
#include "../../src/candidate/ToolbarIcons.h"
#include "../../src/candidate/ToolbarLayout.h"
#include "../../compat/WindowsDpi.h"
#include <cmath>

namespace msime::settings::win7 {

// 只绘制固定状态，不启动悬浮工具栏、不改变当前输入会话。
class ToolbarPreview {
public:
  void preferences(const Json& value) { preferences_ = value; }
  void theme(theme_data::Preview value) { theme_ = std::move(value); }
  std::wstring description() const {
    const auto settings = msime::windows::floating_toolbar_settings(preferences_);
    if (!settings) return L"无法读取工具栏设置。";
    const auto metrics = msime::windows::toolbar_metrics(settings->font_size, false, settings->show_logo);
    const auto count = msime::windows::floating_toolbar_slots(*settings, MSIME_EDITION_HANDWRITING != 0).size();
    const double scale = settings->scale_percent / 100.0;
    return L"预览 · " + std::to_wstring(static_cast<int>(std::ceil(msime::windows::toolbar_content_width(count, metrics) * scale))) +
        L" × " + std::to_wstring(static_cast<int>(std::ceil(metrics.height * scale))) + L" DIP（空间不足时等比缩小）";
  }
  bool draw(const DRAWITEMSTRUCT& draw) const {
    if (draw.CtlID != toolbar_preview) return false;
    using namespace msime::windows;
    const auto dc = draw.hDC;
    const int saved = SaveDC(dc);
    FillRect(dc, &draw.rcItem, GetSysColorBrush(COLOR_BTNFACE));
    const auto settings = floating_toolbar_settings(preferences_);
    if (!settings || !theme_.resolved.is_object()) {
      auto rect = draw.rcItem;
      SetBkMode(dc, TRANSPARENT); SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
      DrawTextW(dc, L"工具栏预览暂不可用，请刷新后再试。", -1, &rect, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
      RestoreDC(dc, saved); return true;
    }
    const auto palette = toolbar_palette(theme_data::palette(theme_));
    const auto flatten = [](CandidateColor value, COLORREF background) {
      const auto channel = [value](float foreground, int behind) { return static_cast<int>(std::clamp(foreground * 255 * value.a + behind * (1 - value.a), 0.0f, 255.0f)); };
      return RGB(channel(value.r, GetRValue(background)), channel(value.g, GetGValue(background)), channel(value.b, GetBValue(background)));
    };
    const auto surface = flatten(palette.surface, GetSysColor(COLOR_BTNFACE));
    const auto foreground = flatten(palette.text, surface);
    const auto metrics = toolbar_metrics(settings->font_size, false, settings->show_logo);
    const auto slots = floating_toolbar_slots(*settings, MSIME_EDITION_HANDWRITING != 0);
    const double content_width = toolbar_content_width(slots.size(), metrics);
    const double natural = settings->scale_percent / 100.0 * compat::window_dpi(draw.hwndItem) / 96.0;
    const double unit = std::max(0.01, std::min({natural, (draw.rcItem.right - draw.rcItem.left - 16.0) / content_width,
        (draw.rcItem.bottom - draw.rcItem.top - 16.0) / metrics.height}));
    const int left = draw.rcItem.left + 8, top = draw.rcItem.top + 8;
    const auto rectangle = [&](ToolbarRect cell) { return RECT{left + static_cast<LONG>(std::lround(cell.left * unit)),
        top + static_cast<LONG>(std::lround(cell.top * unit)), left + static_cast<LONG>(std::lround(cell.right * unit)),
        top + static_cast<LONG>(std::lround(cell.bottom * unit))}; };
    const auto card = rectangle(toolbar_card(slots.size(), metrics));
    const auto brush = CreateSolidBrush(surface);
    const auto pen = CreatePen(PS_SOLID, std::max(1, static_cast<int>(std::lround(unit))), flatten(palette.border, surface));
    const auto previous_brush = SelectObject(dc, brush), previous_pen = SelectObject(dc, pen);
    const int diameter = static_cast<int>(std::lround(palette.radius * 2 * unit));
    RoundRect(dc, card.left, card.top, card.right, card.bottom, diameter, diameter);
    SelectObject(dc, previous_brush); SelectObject(dc, previous_pen); DeleteObject(brush); DeleteObject(pen);
    SetBkMode(dc, TRANSPARENT); SetTextColor(dc, foreground);
    const auto logo = rectangle(toolbar_logo(metrics));
    if (settings->show_logo) {
      const auto icon = reinterpret_cast<HICON>(LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(101), IMAGE_ICON,
          logo.right - logo.left, logo.bottom - logo.top, LR_DEFAULTCOLOR));
      if (icon) { DrawIconEx(dc, logo.left, logo.top, icon, logo.right - logo.left, logo.bottom - logo.top, 0, nullptr, DI_NORMAL); DestroyIcon(icon); }
    } else {
      const auto grip = CreateSolidBrush(flatten(palette.number, surface));
      const auto prior = SelectObject(dc, grip); const auto no_pen = SelectObject(dc, GetStockObject(NULL_PEN));
      const int radius = std::max(1, static_cast<int>(std::lround(unit)));
      for (int row = -1; row <= 1; ++row) for (int column = -1; column <= 1; column += 2) {
        const int x = (logo.left + logo.right) / 2 + static_cast<int>(std::lround(column * 2 * unit));
        const int y = (logo.top + logo.bottom) / 2 + static_cast<int>(std::lround(row * 5 * unit));
        Ellipse(dc, x - radius, y - radius, x + radius, y + radius);
      }
      SelectObject(dc, prior); SelectObject(dc, no_pen); DeleteObject(grip);
    }
    if (!slots.empty()) {
      auto divider = rectangle(toolbar_divider(metrics));
      const auto divider_brush = CreateSolidBrush(flatten(palette.divider.value_or(palette.border), surface)); FillRect(dc, &divider, divider_brush); DeleteObject(divider_brush);
    }
    ToolbarLanguageState language;
    language.scheme = scheme::scheme_from_name(preferences_.value("scheme", std::string(MSIME_EDITION_DEFAULT_SCHEME)));
    language.mode = scheme::input_mode(language.scheme);
    const bool chinese = preferences_.value("default_ime_mode", std::string("chinese")) != "english";
    for (size_t index = 0; index < slots.size(); ++index) {
      const int id = slots[index];
      const bool state = id == kToolbarLanguage ? chinese :
          id == kToolbarFullwidth ? preferences_.value("character_width", std::string("halfwidth")) == "fullwidth" :
          id == kToolbarCharacterSet ? preferences_.value("traditional_chinese_output", false) :
          id == kToolbarPunctuation ? punctuation::toolbar_chinese_punctuation(chinese,
              control_text::wide(preferences_.value("punctuation_lock", std::string("follow")))) : true;
      const auto icon = toolbar_icon(id, state, language);
      const auto font = CreateFontW(-std::max(1, static_cast<int>(std::lround(metrics.icon * toolbar_glyph_scale * unit))), 0, 0, 0,
          FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY,
          DEFAULT_PITCH, icon.family ? icon.family : L"Microsoft YaHei");
      const auto old_font = font ? SelectObject(dc, font) : nullptr;
      auto cell = rectangle(toolbar_cell(index, metrics));
      // Win7 不保证存在 Fluent 图标字体；使用与正式工具栏相同的文字回退。
      DrawTextW(dc, icon.fallback, -1, &cell, DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
      if (font) { SelectObject(dc, old_font); DeleteObject(font); }
    }
    RestoreDC(dc, saved); return true;
  }
private:
  Json preferences_ = Json::object();
  theme_data::Preview theme_;
};
} // namespace msime::settings::win7
