#pragma once
#include "CandidatePalette.h"
#include "CandidateSkinLayout.h"
#include <algorithm>
#include <cstdio>
#include <optional>
#include <string>

namespace msime::windows {
// 皮肤 `[candidate_window] backdrop = "acrylic" | "mica"` 在候选卡片背后垫的系统材质。规则照 msime-windows c0a9d371 的 candidate_skin_scene.cpp 与 candidate_presenter.cpp：亚克力模糊卡片后面的窗口，云母模糊桌面壁纸（Windows 11 才有，更早的系统按亚克力画，由 DeviceResources 在运行时看壁纸模糊画刷建不建得起来）；材质按 Fluent 的配方叠三层，模糊的来源、亮度层（`backdrop_luminosity`）、色调（`surface` 的 RGB，不透明度 `backdrop_opacity`，没写时取 `surface` 自带的 alpha）。皮肤要材质却画不出来时（九宫格面板盖住了卡片除外），`surface` 补成不透明，免得为材质准备的半透明底色直接透出后面的窗口。这里不碰 Win32 和 Direct2D，系统状态由调用方读好传进来，规则在没有 Windows 的机器上也能测。
enum class CandidateBackdropSource { windows_behind, wallpaper };
// 这一帧卡片背后是什么。`active` 是真的垫了材质，其余都是画不画、为什么不画。
enum class CandidateBackdropState {
  // 皮肤没要材质（没写、`none`，或没有皮肤）。
  none,
  active,
  // 九宫格面板（`border_image`）盖住整张卡片，垫材质没有意义，同 msime-windows；底色照常。
  panel_image,
  // 系统「透明效果」关着（设置 → 个性化 → 颜色），系统材质一律退回纯色。
  transparency_off,
  // 高对比度主题：系统自己的材质在这时也都换成纯色，卡片要用主题给的不透明底色。
  high_contrast,
  // 这台机器建不起 Windows.UI.Composition 的材质（低于 Windows 10 1903、远程会话拒绝等），本次运行不再尝试。
  unavailable,
};
struct CandidateBackdropMaterial {
  CandidateBackdropSource source = CandidateBackdropSource::windows_behind;
  // 色调，只用 RGB。
  float tint_r = 0.0f, tint_g = 0.0f, tint_b = 0.0f;
  float tint_opacity = 0.0f;
  float luminosity_opacity = 0.0f;
  friend bool operator==(const CandidateBackdropMaterial &left, const CandidateBackdropMaterial &right) {
    return left.source == right.source && left.tint_r == right.tint_r && left.tint_g == right.tint_g &&
           left.tint_b == right.tint_b && left.tint_opacity == right.tint_opacity &&
           left.luminosity_opacity == right.luminosity_opacity;
  }
  friend bool operator!=(const CandidateBackdropMaterial &left, const CandidateBackdropMaterial &right) {
    return !(left == right);
  }
};
// 调用方读到的系统状态。
struct CandidateBackdropSystem {
  bool transparency_enabled = true;
  bool high_contrast = false;
  // 之前一次建合成失败过（DeviceResources::EnsureForComposition 在材质模式下返回 false）。
  bool composition_unavailable = false;
};
struct CandidateBackdropPlan {
  CandidateBackdropState state = CandidateBackdropState::none;
  // 只在 active 时有意义。
  CandidateBackdropMaterial material;
  bool active() const { return state == CandidateBackdropState::active; }
  // 皮肤要材质、而且没有被九宫格面板盖住：画不出来时底色要补成不透明。
  bool wanted() const {
    return state != CandidateBackdropState::none && state != CandidateBackdropState::panel_image;
  }
};
// 材质参数：`backdrop_opacity` 是色调的不透明度，没写时取 `surface` 的 alpha；`backdrop_luminosity` 是亮度层的不透明度，没写时云母 1、亚克力 0.85（msime-windows 的缺省）。两者都夹到 0-1。
inline CandidateBackdropMaterial candidate_backdrop_material(CandidateSkinBackdrop backdrop, const CandidateColor &surface,
                                                             std::optional<float> opacity,
                                                             std::optional<float> luminosity) {
  const bool mica = backdrop == CandidateSkinBackdrop::mica;
  CandidateBackdropMaterial material;
  material.source = mica ? CandidateBackdropSource::wallpaper : CandidateBackdropSource::windows_behind;
  material.tint_r = surface.r;
  material.tint_g = surface.g;
  material.tint_b = surface.b;
  material.tint_opacity = std::clamp(opacity.value_or(surface.a), 0.0f, 1.0f);
  material.luminosity_opacity = std::clamp(luminosity.value_or(mica ? 1.0f : 0.85f), 0.0f, 1.0f);
  return material;
}
// 先后照 msime-windows：皮肤没要材质 → 面板贴图盖住 → 建不起合成（这次运行一直如此）→ 透明效果关闭 → 高对比度。
inline CandidateBackdropPlan candidate_backdrop_plan(CandidateSkinBackdrop backdrop, bool panel_image,
                                                     const CandidatePalette &palette,
                                                     const CandidateBackdropSystem &system) {
  CandidateBackdropPlan plan;
  if (backdrop == CandidateSkinBackdrop::none)
    return plan;
  if (panel_image) {
    plan.state = CandidateBackdropState::panel_image;
    return plan;
  }
  if (system.composition_unavailable)
    plan.state = CandidateBackdropState::unavailable;
  else if (!system.transparency_enabled)
    plan.state = CandidateBackdropState::transparency_off;
  else if (system.high_contrast)
    plan.state = CandidateBackdropState::high_contrast;
  else
    plan.state = CandidateBackdropState::active;
  if (plan.active())
    plan.material = candidate_backdrop_material(backdrop, palette.surface, palette.backdrop_opacity,
                                                palette.backdrop_luminosity);
  return plan;
}
// 卡片自己画的底色的不透明度：垫了材质时色调已经在合成层里，卡片不再叠底色（0）；要材质却画不出来时补成不透明（candidate_skin_surface_alpha）；其余照旧。用户的不透明度之后照常作用在它上面。
inline float candidate_backdrop_surface_alpha(const CandidateBackdropPlan &plan, float alpha) {
  if (plan.active())
    return 0.0f;
  return plan.wanted() ? candidate_skin_surface_alpha(CandidateSkinBackdrop::acrylic, alpha) : alpha;
}
// 材质覆盖的圆角矩形，DIP。卡片随进入动效平移、缩放，随打字特效抖动，材质跟着卡片走：先平移 (translate_x, translate_y)，再以 (center_x, center_y) 为中心缩放 scale，和 paint() 给渲染目标设的变换同一个顺序。
struct CandidateBackdropRect {
  float left = 0.0f, top = 0.0f, right = 0.0f, bottom = 0.0f, radius = 0.0f;
};
inline CandidateBackdropRect candidate_backdrop_rect(float left, float top, float right, float bottom, float radius,
                                                     float translate_x, float translate_y, float scale,
                                                     float center_x, float center_y) {
  const auto x = [&](float value) { return center_x + scale * (value + translate_x - center_x); };
  const auto y = [&](float value) { return center_y + scale * (value + translate_y - center_y); };
  return CandidateBackdropRect{x(left), y(top), x(right), y(bottom), (std::max)(radius, 0.0f) * scale};
}
// 诊断日志和截图工具里的一行说明。wallpaper_available 是 DeviceResources 在这台机器上建没建起壁纸模糊画刷。
inline std::string candidate_backdrop_label(const CandidateBackdropPlan &plan, bool wallpaper_available) {
  switch (plan.state) {
  case CandidateBackdropState::none:
    return "none";
  case CandidateBackdropState::panel_image:
    return "none (the border_image panel covers the card)";
  case CandidateBackdropState::transparency_off:
    return "opaque surface (transparency effects are off)";
  case CandidateBackdropState::high_contrast:
    return "opaque surface (high contrast)";
  case CandidateBackdropState::unavailable:
    return "opaque surface (system backdrop unavailable)";
  case CandidateBackdropState::active:
    break;
  }
  const auto &material = plan.material;
  const auto channel = [](float value) {
    return static_cast<unsigned>(std::clamp(value, 0.0f, 1.0f) * 255.0f + 0.5f);
  };
  char detail[96];
  std::snprintf(detail, sizeof(detail), ", tint #%02X%02X%02X at %.2f, luminosity %.2f", channel(material.tint_r),
                channel(material.tint_g), channel(material.tint_b), static_cast<double>(material.tint_opacity),
                static_cast<double>(material.luminosity_opacity));
  if (material.source == CandidateBackdropSource::windows_behind)
    return std::string("acrylic (blurred windows behind)") + detail;
  return std::string(wallpaper_available ? "mica (blurred wallpaper)"
                                         : "mica drawn as acrylic (no blurred wallpaper on this Windows)") +
         detail;
}
} // namespace msime::windows
