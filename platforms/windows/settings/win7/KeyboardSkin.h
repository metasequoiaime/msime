#pragma once

#include "DataUrlImage.h"
#include <objidl.h>
#include <gdiplus.h>
#include <algorithm>
#include <cmath>

namespace msime::settings::win7::keyboard_skin {
inline bool start() {
  struct Runtime {
    ULONG_PTR token = 0;
    Runtime() { Gdiplus::GdiplusStartupInput input; if (Gdiplus::GdiplusStartup(&token,&input,nullptr) != Gdiplus::Ok) token = 0; }
    ~Runtime() { if (token) Gdiplus::GdiplusShutdown(token); }
  };
  static const Runtime runtime;
  return runtime.token != 0;
}
inline Gdiplus::Color rgb(uint32_t value, double alpha = 1) {
  return Gdiplus::Color(static_cast<BYTE>(std::clamp(alpha,0.0,1.0) * 255),static_cast<BYTE>(value >> 16),static_cast<BYTE>(value >> 8),static_cast<BYTE>(value));
}
inline uint32_t mix(uint32_t a, uint32_t b, double amount) {
  uint32_t result = 0;
  for (int shift : {0,8,16}) result |= static_cast<uint32_t>(((a >> shift) & 255) * (1 - amount) + ((b >> shift) & 255) * amount) << shift;
  return result;
}
inline uint32_t readable(uint32_t color) {
  return (((color >> 16) & 255) * 299 + ((color >> 8) & 255) * 587 + (color & 255) * 114) > 150000 ? 0x17251D : 0xFFFFFF;
}
struct Look {
  Json design;
  uint32_t background = 0xE4E6EA, key = 0xFFFFFF, text = 0x1A1A1A, accent = 0x006446, action = 0xC8CCD3;
  theme_data::Image photo;
  bool image_failed = false;
  double number(const char* name, double fallback) const { const auto item = design.find(name); return item != design.end() && item->is_number() ? item->get<double>() : fallback; }
  std::string word(const char* name, const char* fallback) const { const auto item = design.find(name); return item != design.end() && item->is_string() ? item->get<std::string>() : fallback; }
  bool mono() const { return design.is_object() && design.value("monospaced",false); }
  bool flag(const char* name, bool fallback = false) const { const auto item = design.find(name); return item != design.end() && item->is_boolean() ? item->get<bool>() : fallback; }
  static Look custom(Json value) {
    Look result; result.design = std::move(value);
    if (!result.design.is_object()) return result;
    const auto color = [&](const char* key, uint32_t fallback) { const auto field = result.design.find(key); return field != result.design.end() && field->is_number_integer() ? field->get<uint32_t>() : fallback; };
    result.background = color("background",result.background); result.key = color("keyBackground",result.key);
    result.text = color("keyForeground",result.text); result.accent = color("accent",result.accent); result.action = color("actionBackground",result.action);
    const auto item = result.design.find("photo");
    if (item != result.design.end() && item->is_string() && !item->get_ref<const std::string&>().empty()) {
      // 原图仍由共享层验证和解码，原生绘制器不另引入一套图像格式策略。
      const auto& encoded = item->get_ref<const std::string&>();
      result.photo = image_data::decode("data:image/jpeg;base64," + encoded,512000);
      if (result.photo.bgra.empty()) result.photo = image_data::decode("data:image/png;base64," + encoded,512000);
      result.image_failed = result.photo.bgra.empty();
    }
    return result;
  }
  static Look resolved(const Json& preferences, const Json& theme) {
    if (preferences.is_object() && preferences.value("global_theme",std::string{}) == "custom") {
      const auto custom = preferences.find("custom_theme");
      if (custom != preferences.end() && custom->is_object() && custom->contains("keyboard") && custom->at("keyboard").is_object()) return Look::custom(custom->at("keyboard"));
    }
    Look result;
    const auto palette = theme.is_object() ? theme.value("keyboard",Json::object()) : Json::object();
    const auto color = [&](const Json& object, const char* key, uint32_t fallback) {
      const auto item = object.find(key); if (item == object.end() || !item->is_string()) return fallback;
      const auto text = item->get<std::string>(); if (text.size() != 7 || text.front() != '#') return fallback;
      try { size_t length = 0; const auto value = std::stoul(text.substr(1),&length,16); return length == 6 ? static_cast<uint32_t>(value) : fallback; } catch (...) { return fallback; }
    };
    result.background = color(palette,"background",result.background); result.key = color(palette,"key",result.key);
    result.text = color(palette,"text",result.text); result.action = color(palette,"function_key",result.action); result.accent = color(theme,"accent",result.accent);
    return result;
  }
};
inline void shape(Gdiplus::GraphicsPath& path, Gdiplus::RectF area, const Look& look, float scale) {
  using namespace Gdiplus;
  const auto kind = look.word("keyShape","rounded");
  float radius = static_cast<float>(look.number("cornerRadius",8)) * scale;
  if (kind == "capsule") radius = std::min(area.Width,area.Height) / 2;
  else if (kind == "ticket") radius = std::min(radius,4 * scale);
  if (kind == "pebble") {
    path.AddBezier(area.X + area.Width * .42f,area.Y,area.GetRight(),area.Y - area.Height * .05f,area.GetRight(),area.Y + area.Height * .3f,area.GetRight(),area.Y + area.Height * .44f);
    path.AddBezier(area.GetRight(),area.Y + area.Height * .44f,area.GetRight(),area.GetBottom(),area.X + area.Width * .7f,area.GetBottom(),area.X + area.Width * .52f,area.GetBottom());
    path.AddBezier(area.X + area.Width * .52f,area.GetBottom(),area.X,area.GetBottom(),area.X,area.Y + area.Height * .7f,area.X,area.Y + area.Height * .52f);
    path.AddBezier(area.X,area.Y + area.Height * .52f,area.X,area.Y,area.X + area.Width * .2f,area.Y,area.X + area.Width * .42f,area.Y); path.CloseFigure(); return;
  }
  radius = std::clamp(radius,0.0f,std::min(area.Width,area.Height) / 2);
  if (kind == "ticket") {
    const float r = std::max(.1f,radius), d = r * 2;
    const float notch = std::min(3 * scale,std::min(area.Width,area.Height) / 4), middle = area.Y + area.Height / 2;
    path.AddArc(area.X,area.Y,d,d,180,90); path.AddArc(area.GetRight() - d,area.Y,d,d,270,90);
    path.AddLine(area.GetRight(),area.Y + r,area.GetRight(),middle - notch);
    path.AddArc(area.GetRight() - notch,middle - notch,notch * 2,notch * 2,270,-180);
    path.AddArc(area.GetRight() - d,area.GetBottom() - d,d,d,0,90); path.AddArc(area.X,area.GetBottom() - d,d,d,90,90);
    path.AddLine(area.X,area.GetBottom() - r,area.X,middle + notch);
    path.AddArc(area.X - notch,middle - notch,notch * 2,notch * 2,90,-180); path.CloseFigure(); return;
  }
  if (radius < .1f) { path.AddRectangle(area); return; }
  const float d = radius * 2;
  path.AddArc(area.X,area.Y,d,d,180,90); path.AddArc(area.GetRight() - d,area.Y,d,d,270,90);
  path.AddArc(area.GetRight() - d,area.GetBottom() - d,d,d,0,90); path.AddArc(area.X,area.GetBottom() - d,d,d,90,90); path.CloseFigure();
}
inline void background(Gdiplus::Graphics& graphics, const Gdiplus::RectF& area, const Look& look, float scale) {
  using namespace Gdiplus;
  SolidBrush base(rgb(look.background)); graphics.FillRectangle(&base,area);
  if (look.design.is_object() && look.design.contains("gradientEnd") && look.design.at("gradientEnd").is_number()) {
    LinearGradientBrush gradient(area,rgb(look.background),rgb(look.design.at("gradientEnd").get<uint32_t>()),look.flag("gradientHorizontal") ? LinearGradientModeHorizontal : LinearGradientModeVertical);
    graphics.FillRectangle(&gradient,area);
  }
  if (!look.photo.bgra.empty()) {
    const auto& image = look.photo;
    Bitmap bitmap(static_cast<INT>(image.width),static_cast<INT>(image.height),static_cast<INT>(image.width * 4),PixelFormat32bppPARGB,const_cast<BYTE*>(image.bgra.data()));
    const double zoom = std::max(area.Width / image.width,area.Height / image.height);
    const float source_width = static_cast<float>(area.Width / zoom), source_height = static_cast<float>(area.Height / zoom);
    const float position = static_cast<float>(look.number("photoPosition",.5));
    graphics.SetInterpolationMode(InterpolationModeHighQualityBicubic);
    graphics.DrawImage(&bitmap,area,(image.width - source_width) * position,(image.height - source_height) * position,source_width,source_height,UnitPixel);
    SolidBrush shade(rgb(0,look.number("photoShade",.25))); graphics.FillRectangle(&shade,area);
  }
  const int pattern = static_cast<int>(look.number("pattern",0));
  Pen line(rgb(look.accent,look.number("patternOpacity",.15)),std::max(.5f,scale));
  SolidBrush dot(rgb(look.accent,look.number("patternOpacity",.15)));
  if (pattern == 1) for (float y = area.Y + 8 * scale; y < area.GetBottom(); y += 16 * scale)
    for (float x = area.X + 8 * scale; x < area.GetRight(); x += 16 * scale) graphics.FillEllipse(&dot,x,y,2 * scale,2 * scale);
  else if (pattern == 2) {
    for (float y = area.Y; y < area.GetBottom(); y += 20 * scale) graphics.DrawLine(&line,area.X,y,area.GetRight(),y);
    for (float x = area.X; x < area.GetRight(); x += 20 * scale) graphics.DrawLine(&line,x,area.Y,x,area.GetBottom());
  } else if (pattern == 3) for (float x = area.X - area.Height * 2; x < area.GetRight(); x += 38 * scale)
    graphics.DrawLine(&line,x,area.GetBottom(),x + area.Height * 2,area.Y);
}
inline void key(Gdiplus::Graphics& graphics, Gdiplus::RectF area, const Look& look, bool function, bool selected, float scale) {
  using namespace Gdiplus;
  graphics.SetSmoothingMode(SmoothingModeAntiAlias); area.Inflate(-scale,-scale);
  if (area.Width <= 0 || area.Height <= 0) return;
  const auto material = look.word("keyMaterial","flat");
  const uint32_t color = selected ? look.accent : function ? look.action : look.key;
  const double opacity = function || selected ? 1 : look.number("keyOpacity",1);
  const double shadow = look.number("shadow",0);
  if (shadow > 0 || material == "raised") {
    auto lower = area; lower.Y += (material == "raised" ? 3 : 1) * scale;
    GraphicsPath path; shape(path,lower,look,scale); SolidBrush brush(rgb(material == "raised" ? mix(color,0,.25) : 0,material == "raised" ? opacity : shadow)); graphics.FillPath(&brush,&path);
  }
  GraphicsPath path; shape(path,area,look,scale);
  if (material == "glass" || material == "raised") {
    LinearGradientBrush fill(area,rgb(mix(color,0xFFFFFF,material == "glass" ? .35 : .08),opacity),rgb(color,opacity),LinearGradientModeVertical); graphics.FillPath(&fill,&path);
  } else { SolidBrush fill(rgb(color,opacity)); graphics.FillPath(&fill,&path); }
  const auto clip = graphics.Save(); graphics.SetClip(&path,CombineModeIntersect);
  if (material == "paper") { Pen line(rgb(look.accent,.08),std::max(.5f,scale)); for (float y = area.Y + 5 * scale; y < area.GetBottom(); y += 5 * scale) graphics.DrawLine(&line,area.X,y,area.GetRight(),y); }
  graphics.Restore(clip);
  const float border = static_cast<float>(look.number("borderWidth",0)) * scale;
  if (border > 0) { Pen pen(rgb(static_cast<uint32_t>(look.number("customBorderColor",look.accent))),border); graphics.DrawPath(&pen,&path); }
}
inline void preview(HDC dc, RECT bounds, const Look& look, HFONT font) {
  if (!start()) return;
  const float width = static_cast<float>(bounds.right - bounds.left), height = static_cast<float>(bounds.bottom - bounds.top);
  const float scale = std::max(.05f,std::min(width / 460,height / 170));
  {
    Gdiplus::Graphics graphics(dc); background(graphics,Gdiplus::RectF(static_cast<float>(bounds.left),static_cast<float>(bounds.top),width,height),look,scale);
  }
  HFONT mono = nullptr;
  if (look.mono()) { LOGFONTW description{}; if (GetObjectW(font,sizeof(description),&description)) { wcscpy_s(description.lfFaceName,L"Consolas"); mono = CreateFontIndirectW(&description); } }
  const auto saved = SaveDC(dc); SelectObject(dc,mono ? mono : font); SetBkMode(dc,TRANSPARENT);
  constexpr wchar_t labels[] = L"QWERTYUIOPASDFGHJKLZXCVBNM";
  size_t index = 0;
  for (int row = 0; row < 3; ++row) for (int column = 0; column < (row == 0 ? 10 : row == 1 ? 9 : 7); ++column) {
    RECT area{bounds.left + static_cast<LONG>((8 + column * 44 + row * 10) * scale),bounds.top + static_cast<LONG>((8 + row * 50) * scale),
      bounds.left + static_cast<LONG>((48 + column * 44 + row * 10) * scale),bounds.top + static_cast<LONG>((52 + row * 50) * scale)};
    { Gdiplus::Graphics graphics(dc); key(graphics,Gdiplus::RectF(static_cast<float>(area.left),static_cast<float>(area.top),static_cast<float>(area.right - area.left),static_cast<float>(area.bottom - area.top)),look,false,false,scale); }
    SetTextColor(dc,RGB((look.text >> 16) & 255,(look.text >> 8) & 255,look.text & 255)); DrawTextW(dc,&labels[index++],1,&area,DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
  }
  RestoreDC(dc,saved); if (mono) DeleteObject(mono);
}
}
