#include "CandidateBackdrop.h"
#include "CandidateSkin.h"
#include <cmath>
#include <stdexcept>
#include <string>

using namespace msime::windows;
using Json = nlohmann::json;
namespace {
void require(bool value, const char *what) {
  if (!value)
    throw std::runtime_error(std::string("Candidate backdrop validation failed: ") + what);
}
bool near(double value, double expected) { return std::fabs(value - expected) < 0.001; }
CandidatePalette translucent(float alpha) {
  CandidatePalette palette;
  palette.surface = CandidateColor{0.2f, 0.4f, 0.6f, alpha};
  return palette;
}
} // namespace

int main() {
  const CandidateBackdropSystem system;
  // 皮肤没要材质：什么都不垫，底色照旧。
  {
    const auto plan = candidate_backdrop_plan(CandidateSkinBackdrop::none, false, translucent(0.6f), system);
    require(plan.state == CandidateBackdropState::none && !plan.active() && !plan.wanted(), "none state");
    require(near(candidate_backdrop_surface_alpha(plan, 0.6f), 0.6), "none keeps the surface alpha");
    require(candidate_backdrop_label(plan, true) == "none", "none label");
  }
  // 亚克力：色调取 surface 的 RGB，参数照写的取；卡片不再画底色。
  {
    auto palette = translucent(0.6f);
    palette.backdrop_opacity = 0.72f;
    palette.backdrop_luminosity = 0.5f;
    const auto plan = candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, false, palette, system);
    require(plan.active() && plan.wanted(), "acrylic is active");
    require(plan.material.source == CandidateBackdropSource::windows_behind, "acrylic blurs the windows behind");
    require(near(plan.material.tint_r, 0.2) && near(plan.material.tint_g, 0.4) && near(plan.material.tint_b, 0.6),
            "tint is the surface colour");
    require(near(plan.material.tint_opacity, 0.72) && near(plan.material.luminosity_opacity, 0.5),
            "written parameters are taken");
    require(near(candidate_backdrop_surface_alpha(plan, 0.6f), 0.0), "the card draws no surface over the material");
    require(candidate_backdrop_label(plan, false) ==
                "acrylic (blurred windows behind), tint #336699 at 0.72, luminosity 0.50",
            "acrylic label");
  }
  // 没写参数：色调不透明度取 surface 的 alpha，亮度层亚克力 0.85、云母 1；云母取样壁纸。
  {
    const auto acrylic = candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, false, translucent(0.3f), system);
    require(near(acrylic.material.tint_opacity, 0.3) && near(acrylic.material.luminosity_opacity, 0.85),
            "acrylic defaults");
    const auto mica = candidate_backdrop_plan(CandidateSkinBackdrop::mica, false, translucent(1.0f), system);
    require(mica.active() && mica.material.source == CandidateBackdropSource::wallpaper, "mica samples the wallpaper");
    require(near(mica.material.tint_opacity, 1.0) && near(mica.material.luminosity_opacity, 1.0), "mica defaults");
    require(candidate_backdrop_label(mica, true).rfind("mica (blurred wallpaper), tint #336699", 0) == 0,
            "mica label");
    require(candidate_backdrop_label(mica, false).rfind("mica drawn as acrylic", 0) == 0,
            "mica without a wallpaper brush is acrylic");
  }
  // 越界的参数夹到 0-1（catalog 已经拒绝越界值，这里只是不信任输入）。
  {
    const auto material =
        candidate_backdrop_material(CandidateSkinBackdrop::acrylic, CandidateColor{0, 0, 0, 1}, 1.5f, -0.5f);
    require(near(material.tint_opacity, 1.0) && near(material.luminosity_opacity, 0.0), "parameters are clamped");
  }
  // 回退链：面板贴图盖住卡片时不要材质、底色照旧；其余画不出来时底色补成不透明。
  {
    const auto palette = translucent(0.4f);
    const auto panel = candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, true, palette, system);
    require(panel.state == CandidateBackdropState::panel_image && !panel.wanted(), "panel image wins");
    require(near(candidate_backdrop_surface_alpha(panel, 0.4f), 0.4), "panel image keeps the surface alpha");
    CandidateBackdropSystem off;
    off.transparency_enabled = false;
    const auto transparency = candidate_backdrop_plan(CandidateSkinBackdrop::mica, false, palette, off);
    require(transparency.state == CandidateBackdropState::transparency_off && transparency.wanted() &&
                !transparency.active(),
            "transparency off falls back");
    require(near(candidate_backdrop_surface_alpha(transparency, 0.4f), 1.0), "the fallback surface is opaque");
    require(candidate_backdrop_label(transparency, true) == "opaque surface (transparency effects are off)",
            "transparency label");
    CandidateBackdropSystem contrast;
    contrast.high_contrast = true;
    const auto high = candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, false, palette, contrast);
    require(high.state == CandidateBackdropState::high_contrast &&
                near(candidate_backdrop_surface_alpha(high, 0.4f), 1.0),
            "high contrast falls back");
    CandidateBackdropSystem broken;
    broken.composition_unavailable = true;
    broken.transparency_enabled = false;
    broken.high_contrast = true;
    const auto unavailable = candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, false, palette, broken);
    require(unavailable.state == CandidateBackdropState::unavailable &&
                near(candidate_backdrop_surface_alpha(unavailable, 0.4f), 1.0),
            "an unavailable compositor is reported first");
    CandidateBackdropSystem both;
    both.transparency_enabled = false;
    both.high_contrast = true;
    require(candidate_backdrop_plan(CandidateSkinBackdrop::acrylic, false, palette, both).state ==
                CandidateBackdropState::transparency_off,
            "transparency is checked before high contrast");
  }
  // 材质的矩形跟着卡片的平移和缩放走，圆角一起缩放。
  {
    const auto still = candidate_backdrop_rect(10, 20, 110, 70, 8, 0, 0, 1, 60, 45);
    require(near(still.left, 10) && near(still.top, 20) && near(still.right, 110) && near(still.bottom, 70) &&
                near(still.radius, 8),
            "an unmoved card keeps its rectangle");
    const auto shaken = candidate_backdrop_rect(10, 20, 110, 70, 8, 2, -3, 1, 60, 45);
    require(near(shaken.left, 12) && near(shaken.top, 17) && near(shaken.right, 112) && near(shaken.bottom, 67),
            "translation moves the rectangle");
    const auto scaled = candidate_backdrop_rect(10, 20, 110, 70, 8, 0, 8, 0.5f, 60, 45);
    require(near(scaled.left, 35) && near(scaled.top, 36.5) && near(scaled.right, 85) && near(scaled.bottom, 61.5) &&
                near(scaled.radius, 4),
            "scale applies about the centre after the translation");
    require(near(candidate_backdrop_rect(0, 0, 1, 1, -2, 0, 0, 1, 0, 0).radius, 0), "a negative radius is zero");
  }
  // 清单读取：材质参数按明暗读 `candidate.<mode>`，越界或不是数的当作没写；`borderImage` 标出九宫格面板。
  {
    const Json candidate{{"light", {{"backdrop_opacity", 0.25}, {"backdrop_luminosity", 0.9}}},
                         {"dark", {{"backdrop_opacity", 1.5}, {"backdrop_luminosity", "0.4"}}}};
    const auto light = apply_candidate_skin_extra_slots(CandidatePalette{}, candidate, Json::object(), false);
    require(light.backdrop_opacity && near(*light.backdrop_opacity, 0.25) && light.backdrop_luminosity &&
                near(*light.backdrop_luminosity, 0.9),
            "light parameters are read");
    const auto dark = apply_candidate_skin_extra_slots(CandidatePalette{}, candidate, Json::object(), true);
    require(!dark.backdrop_opacity && !dark.backdrop_luminosity, "invalid dark parameters are ignored");
    const Json plain{{"id", "plain"}, {"backdrop", "acrylic"}};
    Json panel{{"id", "panel"}, {"backdrop", "mica"}, {"canvas", true}};
    panel["borderImage"] = Json{{"source", "panel.png"}};
    const Json catalog{{"packages", Json::array({plain, panel})}};
    const auto plain_layout = candidate_skin_layout(catalog, "plain");
    require(plain_layout.backdrop == CandidateSkinBackdrop::acrylic && !plain_layout.panel_image, "plain package");
    const auto panel_layout = candidate_skin_layout(catalog, "panel");
    require(panel_layout.backdrop == CandidateSkinBackdrop::mica && panel_layout.panel_image, "panel package");
  }
  return 0;
}
