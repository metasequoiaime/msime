#pragma once
#include <algorithm>
#include <cmath>
#include <filesystem>
#include "CandidateCanvas.h"
#include "CandidatePalette.h"
#include "CandidateSkinLayout.h"
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// Reads the package assets (decoration, background, geometry and toolbar palette) from the catalog msime_client_skin_catalog returns. Its main candidate colours come from msime_client_resolve_theme instead (apply_candidate_skin_extra_slots only adds the slots that resolution does not carry), and the assets are drawn exactly when that resolution names the package in candidate_skin. The catalog serializes SkinSummary in camelCase. The shared catalog already validated the manifest; nothing here trusts it further than the bounds below.
// The mascot a package draws above the card, if it has one.
//
// The catalog parses these and the settings preview renders them, but the live
// card never touched any of it - a mascot skin looked right in settings and
// plain while actually typing.
enum class CandidateSkinAlign { left, center, right };
struct CandidateSkinDecoration {
  // Absolute path to the image, empty when the package has none.
  std::wstring image;
  // How far the artwork rises above the card's top edge, in DIPs.
  double top_dip = 0.0;
  // Drawn width in DIPs; the height follows the image's own aspect ratio.
  double width_dip = 0.0;
  // Where along the card's top edge the artwork sits; right is where every host drew it before a manifest could say otherwise.
  CandidateSkinAlign align = CandidateSkinAlign::right;
};
enum class CandidateSkinFit { cover, contain, stretch };
// An image drawn over the card surface and under the candidates, clipped to the card outline. Empty image means none.
struct CandidateSkinBackground {
  std::wstring image;
  CandidateSkinFit fit = CandidateSkinFit::cover;
  float opacity = 1.0f;
};
// One mode of the package's floating toolbar colours: the scan normalizes them to #RRGGBB(AA), and an absent one keeps what the theme derives.
struct CandidateSkinToolbarColors {
  std::optional<std::string> background, border, handle, divider, icon, hover;
};
struct CandidateSkinToolbar {
  std::optional<float> radius;
  CandidateSkinToolbarColors dark, light;
};

// 与共享皮肤清单相同的固定动效预设；Windows 只执行这些安全原语，不读取皮肤提供的脚本。
enum class CandidateSkinAnimationPreset { none, fade, fade_scale, slide_up, spring, glow };
struct CandidateSkinAnimation {
  CandidateSkinAnimationPreset enter = CandidateSkinAnimationPreset::none;
  CandidateSkinAnimationPreset exit = CandidateSkinAnimationPreset::none;
  CandidateSkinAnimationPreset selection = CandidateSkinAnimationPreset::none;
  unsigned duration_ms = 0;
  unsigned stagger_ms = 0;
};
namespace detail {
inline const nlohmann::json *candidate_skin_package(const nlohmann::json &catalog,
                                                    const std::string &id) {
  if (id.empty() || !catalog.is_object() || !catalog.contains("packages") ||
      !catalog.at("packages").is_array())
    return nullptr;
  for (const auto &package : catalog.at("packages"))
    if (package.is_object() && package.contains("id") &&
        package.at("id").is_string() && package.at("id") == id)
      return &package;
  return nullptr;
}
// 新 catalog 提供四层合并后的布局；旧 catalog 继续使用包的原始声明。
inline const nlohmann::json &candidate_skin_render_layout(const nlohmann::json &package) {
  const auto at = package.find("resolvedLayout");
  return at != package.end() && at->is_object() ? *at : package;
}
// A package-relative file as an absolute path, or empty when the value is not a plausible relative path or the file is not there. The catalog already refused a path that escapes the package directory.
inline std::wstring candidate_skin_file(const nlohmann::json &value,
                                        const std::string &id,
                                        const std::filesystem::path &root) {
  if (!value.is_string())
    return {};
  const auto relative = value.get<std::string>();
  if (relative.empty() || relative.size() > 256)
    return {};
  const auto package_root = root / std::filesystem::u8path(id);
  const auto path = package_root / std::filesystem::u8path(relative);
  const auto normalized = path.lexically_normal();
  const auto relative_path = normalized.lexically_relative(package_root);
  if (normalized != path || relative_path.empty() || relative_path.is_absolute() ||
      std::any_of(relative_path.begin(), relative_path.end(), [](const auto &component) {
        return component == std::filesystem::path("..") ||
               component == std::filesystem::path(".");
      }))
    return {};
  for (auto current = normalized; current != package_root;
       current = current.parent_path()) {
    std::error_code status_error;
    const auto status = std::filesystem::symlink_status(current, status_error);
    if (status_error || status.type() == std::filesystem::file_type::symlink)
      return {};
  }
  std::error_code package_error;
  const auto package_status = std::filesystem::symlink_status(package_root, package_error);
  if (package_error || package_status.type() == std::filesystem::file_type::symlink)
    return {};
  std::error_code error;
  if (!std::filesystem::is_regular_file(normalized, error))
    return {};
  return normalized.wstring();
}
// A DIP radius in the manifest's 0-32 range, or none.
inline std::optional<float> candidate_skin_radius(const nlohmann::json &object,
                                                  const char *key) {
  if (!object.is_object() || !object.contains(key) || !object.at(key).is_number())
    return std::nullopt;
  const auto value = object.at(key).get<double>();
  if (!(value >= 0.0) || value > 32.0)
    return std::nullopt;
  return static_cast<float>(value);
}
} // namespace detail
inline CandidateSkinDecoration
candidate_skin_decoration(const nlohmann::json &catalog, const std::string &id,
                          const std::filesystem::path &root) {
  CandidateSkinDecoration decoration;
  const auto *package = detail::candidate_skin_package(catalog, id);
  // decorationImage is set only for a decorated package: the manifest's decoration.image, or its preview.
  if (!package || !package->contains("decorationImage"))
    return decoration;
  const auto top = package->value("decorationTopDip", 0.0);
  const auto width = package->value("decorationWidthDip", 0.0);
  // Both are required: a decoration with no width would draw nothing, and
  // one that does not rise above the card is not a decoration.
  if (!(top > 0.0) || !(width > 0.0) || top > 512.0 || width > 1024.0)
    return decoration;
  auto image = detail::candidate_skin_file(package->at("decorationImage"), id, root);
  if (image.empty())
    return decoration;
  decoration.image = std::move(image);
  decoration.top_dip = top;
  decoration.width_dip = width;
  const auto align = package->value("decorationAlign", std::string("right"));
  decoration.align = align == "left"     ? CandidateSkinAlign::left
                     : align == "center" ? CandidateSkinAlign::center
                                         : CandidateSkinAlign::right;
  return decoration;
}
// The minimum card width a package asks for, in DIPs, or 0 when it asks for
// none. A mascot skin is drawn against a card of a particular width; a narrower
// card makes the decoration overhang it.
inline double candidate_skin_min_width(const nlohmann::json &catalog,
                                       const std::string &id) {
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("minWidthDip") ||
      !package->at("minWidthDip").is_number())
    return 0.0;
  const auto value = package->at("minWidthDip").get<double>();
  // The catalog validates the manifest, but a value this card cannot use is
  // still refused here rather than propagated into the geometry.
  if (!(value > 0.0) || value > 2000.0)
    return 0.0;
  return value;
}
// 卡片圆角优先使用共享层合并后的布局；未提供时沿用原声明或主题。
inline std::optional<float> candidate_skin_corner_radius(const nlohmann::json &catalog,
                                                         const std::string &id) {
  const auto *package = detail::candidate_skin_package(catalog, id);
  return package ? detail::candidate_skin_radius(detail::candidate_skin_render_layout(*package), "cornerRadiusDip")
                 : std::nullopt;
}
namespace detail {
// 一个 0 到 max 之间的数，越界、不是数或没写都是空。共享 catalog 已经按同样的范围校验过清单，这里只是不把读不懂的值带进几何。
inline std::optional<double> candidate_skin_number(const nlohmann::json &object,
                                                   const char *key, double max) {
  if (!object.is_object() || !object.contains(key) || !object.at(key).is_number())
    return std::nullopt;
  const auto value = object.at(key).get<double>();
  if (!std::isfinite(value) || value < 0.0 || value > max)
    return std::nullopt;
  return value;
}
inline std::optional<bool> candidate_skin_flag(const nlohmann::json &object,
                                               const char *key) {
  if (!object.is_object() || !object.contains(key) || !object.at(key).is_boolean())
    return std::nullopt;
  return object.at(key).get<bool>();
}
// `SkinLength` 序列化成 `{"value": 数, "em": 布尔}`；DIP 和 em 各有自己的上限，与 catalog 的 read_length 相同。
inline std::optional<CandidateSkinLength>
candidate_skin_length_value(const nlohmann::json &object, const char *key,
                            double max_dip, double max_em) {
  if (!object.is_object() || !object.contains(key) || !object.at(key).is_object())
    return std::nullopt;
  const auto &length = object.at(key);
  const auto em = candidate_skin_flag(length, "em").value_or(false);
  const auto value = candidate_skin_number(length, "value", em ? max_em : max_dip);
  if (!value)
    return std::nullopt;
  return CandidateSkinLength{*value, em};
}
// 一张 `SkinGeometry`（camelCase）。
inline CandidateSkinGeometry candidate_skin_geometry_table(const nlohmann::json &table) {
  CandidateSkinGeometry geometry;
  if (!table.is_object())
    return geometry;
  geometry.padding = candidate_skin_length_value(table, "padding", 64.0, 4.0);
  geometry.item_gap = candidate_skin_length_value(table, "itemGap", 64.0, 4.0);
  geometry.item_extra_height =
      candidate_skin_length_value(table, "itemExtraHeight", 64.0, 4.0);
  geometry.number_inset = candidate_skin_length_value(table, "numberInset", 64.0, 4.0);
  geometry.item_padding_right =
      candidate_skin_length_value(table, "itemPaddingRight", 64.0, 4.0);
  geometry.item_min_width =
      candidate_skin_length_value(table, "itemMinWidth", 500.0, 40.0);
  geometry.number_gap = candidate_skin_length_value(table, "numberGap", 64.0, 4.0);
  geometry.selected_bar_width =
      candidate_skin_length_value(table, "selectedBarWidth", 16.0, 1.0);
  geometry.number_scale = candidate_skin_number(table, "numberScale", 2.0);
  geometry.selected_bar_height_em =
      candidate_skin_number(table, "selectedBarHeightEm", 2.0);
  geometry.selected_bar_inside = candidate_skin_flag(table, "selectedBarInside");
  geometry.item_corners_follow_card =
      candidate_skin_flag(table, "itemCornersFollowCard");
  geometry.justify_rows = candidate_skin_flag(table, "justifyRows");
  if (table.contains("pageArrowStyle") && table.at("pageArrowStyle").is_string()) {
    const auto style = table.at("pageArrowStyle").get<std::string>();
    if (style == "chevron")
      geometry.page_arrow_style = CandidatePageArrowStyle::chevron;
    else if (style == "triangle")
      geometry.page_arrow_style = CandidatePageArrowStyle::triangle;
  }
  if (table.contains("preeditMarginDip") && table.at("preeditMarginDip").is_object()) {
    const auto &margin = table.at("preeditMarginDip");
    const auto top = candidate_skin_number(margin, "top", 500.0),
               right = candidate_skin_number(margin, "right", 500.0),
               bottom = candidate_skin_number(margin, "bottom", 500.0),
               left = candidate_skin_number(margin, "left", 500.0);
    if (top && right && bottom && left)
      geometry.preedit_margin = CandidateSkinInsets{*top, *right, *bottom, *left};
  }
  return geometry;
}
} // namespace detail
// 包的版式、外框、阴影、材质、翻页箭头、收紧宽度和卷轴展开（见 CandidateSkinLayout.h）。读不懂的值当作没写，卡片保留宿主自己的值。
inline CandidateSkinLayout candidate_skin_layout(const nlohmann::json &catalog,
                                                 const std::string &id) {
  CandidateSkinLayout layout;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package)
    return layout;
  const auto &resolved = detail::candidate_skin_render_layout(*package);
  for (const auto &[key, target] :
       {std::pair{"geometry", &layout.geometry},
        std::pair{"horizontal", &layout.horizontal},
        std::pair{"vertical", &layout.vertical}})
    if (resolved.contains(key))
      *target = detail::candidate_skin_geometry_table(resolved.at(key));
  if (const auto width = detail::candidate_skin_number(resolved, "borderWidthDip", 4.0))
    layout.border_width = static_cast<float>(*width);
  if (const auto radius =
          detail::candidate_skin_number(resolved, "itemCornerRadiusDip", 16.0))
    layout.item_corner_radius = static_cast<float>(*radius);
  if (resolved.contains("shadow") && resolved.at("shadow").is_string()) {
    const auto shadow = resolved.at("shadow").get<std::string>();
    if (shadow == "none")
      layout.shadow = CandidateSkinShadow::none;
    else if (shadow == "soft")
      layout.shadow = CandidateSkinShadow::soft;
    else if (shadow == "strong")
      layout.shadow = CandidateSkinShadow::strong;
  }
  if (package->contains("backdrop") && package->at("backdrop").is_string()) {
    const auto backdrop = package->at("backdrop").get<std::string>();
    if (backdrop == "acrylic")
      layout.backdrop = CandidateSkinBackdrop::acrylic;
    else if (backdrop == "mica")
      layout.backdrop = CandidateSkinBackdrop::mica;
  }
  layout.panel_image = package->contains("borderImage") && package->at("borderImage").is_object();
  layout.page_arrows = detail::candidate_skin_flag(resolved, "pageArrows");
  layout.fit_content = detail::candidate_skin_flag(resolved, "fitContent");
  layout.expand_panel = detail::candidate_skin_flag(resolved, "expandPanel");
  // catalog 只收 2-8 的整数，越界或不是整数时当作没写。
  if (const auto rows = detail::candidate_skin_number(resolved, "expandRows", 8.0);
      rows && *rows >= 2.0 && std::floor(*rows) == *rows)
    layout.expand_rows = static_cast<int>(*rows);
  return layout;
}
namespace detail {
// `sliceDip` / `stretchDip` / `widthDip`：1、2 或 4 个 0-500 的数，与 catalog 的 read_box_values 相同；读不懂时为空。
inline std::optional<std::vector<double>> candidate_skin_box_values(const nlohmann::json &object,
                                                                   const char *key) {
  if (!object.contains(key) || !object.at(key).is_array())
    return std::nullopt;
  const auto &array = object.at(key);
  if (array.size() != 1 && array.size() != 2 && array.size() != 4)
    return std::nullopt;
  std::vector<double> values;
  for (const auto &value : array) {
    if (!value.is_number())
      return std::nullopt;
    const auto number = value.get<double>();
    if (!std::isfinite(number) || number < 0.0 || number > 500.0)
      return std::nullopt;
    values.push_back(number);
  }
  return values;
}
// `SkinInsets`（四个键都在）；上下不超过 max_vertical，左右不超过 max_horizontal，与 catalog 的 read_insets 相同。
inline std::optional<CandidateCanvasEdges>
candidate_skin_canvas_insets(const nlohmann::json &object, const char *key, double max_vertical,
                             double max_horizontal) {
  if (!object.contains(key) || !object.at(key).is_object())
    return std::nullopt;
  const auto &insets = object.at(key);
  const auto top = candidate_skin_number(insets, "top", max_vertical),
             right = candidate_skin_number(insets, "right", max_horizontal),
             bottom = candidate_skin_number(insets, "bottom", max_vertical),
             left = candidate_skin_number(insets, "left", max_horizontal);
  if (!top || !right || !bottom || !left)
    return std::nullopt;
  return CandidateCanvasEdges{*top, *right, *bottom, *left};
}
// -limit 到 limit 之间的数，没写时是 0；不是数或越界时为空。
inline std::optional<double> candidate_skin_offset(const nlohmann::json &object, const char *key,
                                                   double limit) {
  if (!object.contains(key))
    return 0.0;
  if (!object.at(key).is_number())
    return std::nullopt;
  const auto value = object.at(key).get<double>();
  if (!std::isfinite(value) || value < -limit || value > limit)
    return std::nullopt;
  return value;
}
} // namespace detail
// `base = "canvas"` 包的画布（CandidateCanvas.h）：九宫格面板、内容内边距、外留白、行距和自由挂件。图片路径按包内相对路径解析成绝对路径，和装饰图、背景图同一道检查（不出包目录、不经符号链接、文件存在）。catalog 只在 `canvas` 时才给出这些键；读不懂的面板整个不要（版式也不按它的边宽留），读不懂的挂件只丢那一个，读不懂的内边距、外留白和行距当作没写。
inline CandidateSkinCanvas candidate_skin_canvas(const nlohmann::json &catalog,
                                                 const std::string &id,
                                                 const std::filesystem::path &root) {
  CandidateSkinCanvas canvas;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !detail::candidate_skin_flag(*package, "canvas").value_or(false))
    return canvas;
  canvas.active = true;
  if (package->contains("borderImage") && package->at("borderImage").is_object()) {
    const auto &value = package->at("borderImage");
    const auto slice = detail::candidate_skin_box_values(value, "sliceDip");
    auto source = value.contains("source")
                      ? detail::candidate_skin_file(value.at("source"), id, root)
                      : std::wstring{};
    if (slice && !source.empty()) {
      CandidateCanvasBorder border;
      border.source = std::move(source);
      if (value.contains("lightSource"))
        border.light_source = detail::candidate_skin_file(value.at("lightSource"), id, root);
      // 写了 stretch_dip 时它盖过 slice_dip；没写 width_dip 时边宽取（盖过之后的）切片数值。
      const auto stretch = detail::candidate_skin_box_values(value, "stretchDip");
      border.slice = candidate_canvas_box(stretch ? *stretch : *slice);
      const auto width = detail::candidate_skin_box_values(value, "widthDip");
      border.width = width ? candidate_canvas_box(*width) : border.slice;
      border.fill = detail::candidate_skin_flag(value, "fill").value_or(true);
      canvas.border = std::move(border);
    }
  }
  canvas.content_insets =
      detail::candidate_skin_canvas_insets(*package, "contentInsetsDip", 200.0, 500.0);
  canvas.outer_insets =
      detail::candidate_skin_canvas_insets(*package, "outerInsetsDip", 500.0, 500.0);
  canvas.line_gap = detail::candidate_skin_number(*package, "lineGapDip", 100.0);
  if (package->contains("decorations") && package->at("decorations").is_array())
    for (const auto &value : package->at("decorations")) {
      if (canvas.decorations.size() >= 32)
        break;
      if (!value.is_object() || !value.contains("image"))
        continue;
      CandidateCanvasDecoration decoration;
      decoration.image = detail::candidate_skin_file(value.at("image"), id, root);
      const auto anchor = value.contains("anchor") && value.at("anchor").is_string()
                              ? candidate_canvas_anchor(value.at("anchor").get<std::string>())
                              : std::optional<CandidateCanvasAnchor>{CandidateCanvasAnchor::right};
      const auto offset_x = detail::candidate_skin_offset(value, "offsetXDip", 2000.0);
      const auto offset_y = detail::candidate_skin_offset(value, "offsetYDip", 2000.0);
      const auto width = detail::candidate_skin_number(value, "widthDip", 2000.0);
      const auto height = detail::candidate_skin_number(value, "heightDip", 2000.0);
      if (decoration.image.empty() || !anchor || !offset_x || !offset_y || !width ||
          !height || !(*width > 0.0) || !(*height > 0.0))
        continue;
      decoration.anchor = *anchor;
      decoration.offset_x = *offset_x;
      decoration.offset_y = *offset_y;
      decoration.width = *width;
      decoration.height = *height;
      if (value.contains("zIndex")) {
        if (!value.at("zIndex").is_number_integer())
          continue;
        const auto z = value.at("zIndex").get<int64_t>();
        if (z < -1000 || z > 1000)
          continue;
        decoration.z_index = static_cast<int>(z);
      }
      canvas.decorations.push_back(std::move(decoration));
    }
  return canvas;
}
inline CandidateSkinBackground
candidate_skin_background(const nlohmann::json &catalog, const std::string &id,
                          const std::filesystem::path &root) {
  CandidateSkinBackground background;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("background") ||
      !package->at("background").is_object())
    return background;
  const auto &value = package->at("background");
  if (!value.contains("image") || !value.contains("opacity") ||
      !value.at("opacity").is_number())
    return background;
  const auto opacity = value.at("opacity").get<double>();
  if (!(opacity >= 0.0) || opacity > 1.0)
    return background;
  auto image = detail::candidate_skin_file(value.at("image"), id, root);
  if (image.empty())
    return background;
  const auto fit = value.value("fit", std::string("cover"));
  background.image = std::move(image);
  background.fit = fit == "contain"   ? CandidateSkinFit::contain
                   : fit == "stretch" ? CandidateSkinFit::stretch
                                      : CandidateSkinFit::cover;
  background.opacity = static_cast<float>(opacity);
  return background;
}
inline CandidateSkinToolbar candidate_skin_toolbar(const nlohmann::json &catalog,
                                                   const std::string &id) {
  CandidateSkinToolbar toolbar;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("toolbar") || !package->at("toolbar").is_object())
    return toolbar;
  const auto &value = package->at("toolbar");
  toolbar.radius = detail::candidate_skin_radius(value, "cornerRadiusDip");
  auto colors = [&value](const char *mode) {
    CandidateSkinToolbarColors colors;
    if (!value.contains(mode) || !value.at(mode).is_object())
      return colors;
    const auto &palette = value.at(mode);
    auto read = [&palette](const char *key, std::optional<std::string> &target) {
      // The shared layer only emits #RRGGBB or #RRGGBBAA here.
      if (palette.contains(key) && palette.at(key).is_string() &&
          palette.at(key).get<std::string>().size() <= 16)
        target = palette.at(key).get<std::string>();
    };
    read("background", colors.background);
    read("border", colors.border);
    read("handle", colors.handle);
    read("divider", colors.divider);
    read("icon", colors.icon);
    read("hover", colors.hover);
    return colors;
  };
  toolbar.dark = colors("dark");
  toolbar.light = colors("light");
  return toolbar;
}

// 只叠加共享主题解析（msime_client_resolve_theme）不带的候选颜色槽位：表面、边框、文字、编号、强调、选中、悬停、释义和选中竖条开关已经按「底、皮肤包、取色器」的顺序解析好，这里不再覆盖。`pickers` 是自定义主题的 `custom_theme.candidate_colors`，取色器仍然优先：设了文字色时不叠皮肤的候选文字和预编辑文字，设了选中色时不叠皮肤的选中文字、选中编号和选中释义，保留解析层按亮度推出的可读颜色。画不出的值保持宿主默认。
inline CandidatePalette apply_candidate_skin_extra_slots(
    CandidatePalette palette, const nlohmann::json &candidate,
    const nlohmann::json &pickers, bool dark);
inline CandidatePalette apply_candidate_skin_extra_slots(
    CandidatePalette palette, const nlohmann::json &catalog,
    const std::string &id, const nlohmann::json &pickers, bool dark) {
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("candidate") ||
      !package->at("candidate").is_object())
    return palette;
  return apply_candidate_skin_extra_slots(palette, package->at("candidate"),
                                          pickers, dark);
}

inline CandidatePalette apply_candidate_skin_extra_slots(
    CandidatePalette palette, const nlohmann::json &candidate,
    const nlohmann::json &pickers, bool dark) {
  if (!candidate.is_object())
    return palette;
  const auto mode = dark ? "dark" : "light";
  if (!candidate.contains(mode) || !candidate.at(mode).is_object())
    return palette;
  const auto &colors = candidate.at(mode);
  auto picked = [&pickers](const char *key) {
    return pickers.is_object() && pickers.contains(key) &&
           pickers.at(key).is_string();
  };
  // Only a value that parses is drawn; anything else leaves the host's derived colour.
  auto apply = [&colors](const char *key, CandidateColor &target) {
    if (colors.contains(key) && colors.at(key).is_string())
      if (auto parsed = try_parse_css_color(colors.at(key).get<std::string>()))
        target = *parsed;
  };
  auto optional = [&colors](const char *key,
                            std::optional<CandidateColor> &target) {
    if (colors.contains(key) && colors.at(key).is_string())
      if (auto parsed = try_parse_css_color(colors.at(key).get<std::string>()))
        target = *parsed;
  };
  if (!picked("text")) {
    optional("candidate_text", palette.candidate_text);
    optional("preedit_text", palette.preedit_text);
  }
  if (!picked("selected")) {
    apply("selected_text", palette.selected_text);
    apply("selected_number", palette.selected_number);
    optional("selected_translation", palette.selected_translation);
  }
  optional("preedit_caret", palette.preedit_caret);
  optional("selected_bar", palette.selected_bar);
  optional("preedit_background", palette.preedit_background);
  optional("preedit_divider", palette.preedit_divider);
  optional("page_arrow_divider", palette.page_arrow_divider);
  // 材质参数是 0-1 的数，catalog 已经校验过范围；读不懂的当作没写。
  auto fraction = [&colors](const char *key, std::optional<float> &target) {
    if (colors.contains(key) && colors.at(key).is_number()) {
      const auto value = colors.at(key).get<double>();
      if (value >= 0.0 && value <= 1.0)
        target = static_cast<float>(value);
    }
  };
  fraction("backdrop_opacity", palette.backdrop_opacity);
  fraction("backdrop_luminosity", palette.backdrop_luminosity);
  if (colors.contains("menu") && colors.at("menu").is_object()) {
    const auto &menu = colors.at("menu");
    auto menu_color = [&menu](const char *key, CandidateColor &target) {
      if (menu.contains(key) && menu.at(key).is_string())
        if (auto parsed = try_parse_css_color(menu.at(key).get<std::string>()))
          target = *parsed;
    };
    menu_color("background", palette.menu_fill);
    menu_color("border", palette.menu_border);
    menu_color("text", palette.menu_text);
    menu_color("hover", palette.menu_hover);
  }
  return palette;
}

// The faces the card draws with: a skin's font_family goes first and the user's configured face drops to the head of the fallback chain, as msime-windows' candidate_skin_scene does. `skin` is empty when the package names no font or names one that is not installed, and then the configured chain is used as it is.
struct CandidateFontChain {
  std::wstring primary;
  std::vector<std::wstring> fallback;
};
inline CandidateFontChain
candidate_skin_font_chain(const std::wstring &configured,
                          const std::vector<std::wstring> &configured_fallback,
                          const std::wstring &skin) {
  if (skin.empty() || skin == configured)
    return {configured, configured_fallback};
  CandidateFontChain chain{skin, {configured}};
  for (const auto &family : configured_fallback)
    if (family != skin && std::find(chain.fallback.begin(), chain.fallback.end(),
                                    family) == chain.fallback.end())
      chain.fallback.push_back(family);
  return chain;
}

inline CandidateSkinAnimationPreset candidate_skin_animation_preset(
    const nlohmann::json &value, const char *key,
    CandidateSkinAnimationPreset fallback) {
  if (!value.is_object() || !value.contains(key) || !value.at(key).is_string())
    return fallback;
  const auto preset = value.at(key).get<std::string>();
  if (preset == "fade") return CandidateSkinAnimationPreset::fade;
  if (preset == "fade_scale") return CandidateSkinAnimationPreset::fade_scale;
  if (preset == "slide_up") return CandidateSkinAnimationPreset::slide_up;
  if (preset == "spring") return CandidateSkinAnimationPreset::spring;
  if (preset == "glow") return CandidateSkinAnimationPreset::glow;
  return preset == "none" ? CandidateSkinAnimationPreset::none : fallback;
}

inline CandidateSkinAnimation candidate_skin_animation(const nlohmann::json &catalog,
                                                       const std::string &id) {
  CandidateSkinAnimation animation;
  const auto *package = detail::candidate_skin_package(catalog, id);
  if (!package || !package->contains("schemaVersion") ||
      !package->at("schemaVersion").is_number_integer() ||
      package->at("schemaVersion").get<int64_t>() != 2)
    return animation;
  if (!package->contains("animation") || !package->at("animation").is_object()) {
    animation.enter = CandidateSkinAnimationPreset::fade_scale;
    animation.exit = CandidateSkinAnimationPreset::fade;
    animation.selection = CandidateSkinAnimationPreset::spring;
    animation.duration_ms = 180;
    animation.stagger_ms = 14;
    return animation;
  }
  const auto &value = package->at("animation");
  animation.enter = candidate_skin_animation_preset(
      value, "enter", CandidateSkinAnimationPreset::fade_scale);
  animation.exit = candidate_skin_animation_preset(
      value, "exit", CandidateSkinAnimationPreset::fade);
  animation.selection = candidate_skin_animation_preset(
      value, "selection", CandidateSkinAnimationPreset::spring);
  if (value.contains("durationMs") && value.at("durationMs").is_number_integer()) {
    const auto duration = value.at("durationMs").get<int64_t>();
    if (duration >= 40 && duration <= 600)
      animation.duration_ms = static_cast<unsigned>(duration);
  }
  if (value.contains("staggerMs") && value.at("staggerMs").is_number_integer()) {
    const auto stagger = value.at("staggerMs").get<int64_t>();
    if (stagger >= 0 && stagger <= 80) animation.stagger_ms = static_cast<unsigned>(stagger);
  }
  if (animation.duration_ms == 0) animation.duration_ms = 180;
  return animation;
}
// The toolbar palette with a package's own toolbar colours and radius drawn over what the theme derives: background is the bar surface, handle the drag strip (the accent there), divider the separator, icon the glyphs, hover the hovered button.
inline CandidatePalette apply_toolbar_skin(CandidatePalette palette,
                                           const CandidateSkinToolbar &toolbar,
                                           bool dark) {
  const auto &colors = dark ? toolbar.dark : toolbar.light;
  auto apply = [](const std::optional<std::string> &value, CandidateColor &target) {
    if (value && !value->empty())
      target = parse_css_color(*value, target);
  };
  apply(colors.background, palette.surface);
  apply(colors.border, palette.border);
  apply(colors.handle, palette.accent);
  apply(colors.icon, palette.text);
  apply(colors.hover, palette.hover);
  if (colors.divider && !colors.divider->empty())
    palette.divider = parse_css_color(*colors.divider, palette.border);
  if (toolbar.radius)
    palette.radius = *toolbar.radius;
  return palette;
}
// The left edge of a decoration `width` wide over a card spanning [card_left, card_right], kept `pad` in from the side it is aligned to and never left of the window.
inline float candidate_decoration_left(CandidateSkinAlign align, float card_left,
                                       float card_right, float pad, float width) {
  const float left = align == CandidateSkinAlign::left ? card_left + pad
                     : align == CandidateSkinAlign::center
                         ? (card_left + card_right - width) / 2.0f
                         : card_right - pad - width;
  return (std::max)(0.0f, left);
}
struct CandidateSkinRect {
  float left = 0.0f, top = 0.0f, right = 0.0f, bottom = 0.0f;
};
// Where the mascot is drawn: `width` wide at the image's own aspect ratio, its bottom `pad_y` below the card's top edge so it overhangs into the card, aligned along that edge as candidate_decoration_left places it. The window holds only the `band` above the card plus that overlap, so an image taller than band + pad_y is scaled down uniformly - narrower as well as shorter, keeping its aligned edge and its bottom - rather than squashed into the space. Nothing is drawn for an image or a width with no size.
inline std::optional<CandidateSkinRect>
candidate_decoration_rect(CandidateSkinAlign align, float card_left,
                          float card_right, float card_top, float pad_x,
                          float pad_y, float band, float width,
                          float natural_width, float natural_height) {
  if (!(width > 0.0f) || !(natural_width > 0.0f) || !(natural_height > 0.0f))
    return std::nullopt;
  const float room = (std::max)(0.0f, band + pad_y);
  float height = width * (natural_height / natural_width);
  if (height > room) {
    width *= room / height;
    height = room;
  }
  if (!(width > 0.0f) || !(height > 0.0f))
    return std::nullopt;
  const float left =
      candidate_decoration_left(align, card_left, card_right, pad_x, width);
  const float bottom = card_top + pad_y;
  return CandidateSkinRect{left, bottom - height, left + width, bottom};
}
// Where a background image of `natural` size is drawn in `card`, and which part of the image: cover crops the image's overflow to fill the card, contain letterboxes the whole image inside it, stretch scales each axis to the card.
struct CandidateSkinBackgroundRects {
  CandidateSkinRect destination, source;
};
inline std::optional<CandidateSkinBackgroundRects>
candidate_background_rects(CandidateSkinFit fit, const CandidateSkinRect &card,
                           float natural_width, float natural_height) {
  const float width = card.right - card.left, height = card.bottom - card.top;
  if (!(width > 0.0f) || !(height > 0.0f) || !(natural_width > 0.0f) ||
      !(natural_height > 0.0f))
    return std::nullopt;
  CandidateSkinBackgroundRects rects{card, {0.0f, 0.0f, natural_width, natural_height}};
  if (fit == CandidateSkinFit::stretch)
    return rects;
  const float scale_x = width / natural_width, scale_y = height / natural_height;
  if (fit == CandidateSkinFit::cover) {
    const float scale = (std::max)(scale_x, scale_y);
    const float visible_width = width / scale, visible_height = height / scale;
    rects.source.left = (natural_width - visible_width) / 2.0f;
    rects.source.top = (natural_height - visible_height) / 2.0f;
    rects.source.right = rects.source.left + visible_width;
    rects.source.bottom = rects.source.top + visible_height;
    return rects;
  }
  const float scale = (std::min)(scale_x, scale_y);
  const float drawn_width = natural_width * scale, drawn_height = natural_height * scale;
  rects.destination.left = card.left + (width - drawn_width) / 2.0f;
  rects.destination.top = card.top + (height - drawn_height) / 2.0f;
  rects.destination.right = rects.destination.left + drawn_width;
  rects.destination.bottom = rects.destination.top + drawn_height;
  return rects;
}
} // namespace msime::windows
