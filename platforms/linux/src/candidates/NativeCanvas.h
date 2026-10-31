#pragma once

#include "CandidatePronunciation.h"
#include "CandidateRowText.h"
#include "NativeCanvasAnimation.h"
#include "../../../../shared/contracts/candidate_decoration_overlay.h"
#include "../../../../shared/candidate/CandidateChromeMetrics.h"
#include "../../../../shared/candidate/CandidateDecoration.h"
#include "../../../../shared/candidate/CandidateExpandedGrid.h"
#include "../../../../shared/candidate/CandidateItemLayout.h"
#include "../../../../shared/candidate/CandidateMotionRules.h"
#include "../../../../shared/candidate/CandidatePlacement.h"
#include <pango/pangocairo.h>
#include <functional>
#include <limits>
#include <map>
#include <vector>

namespace msime::linux_host {

struct NativeCanvasRect {
  double x = 0, y = 0, width = 0, height = 0;
  bool contains(double px, double py) const {
    return px >= x && py >= y && px < x + width && py < y + height;
  }
};
struct NativeCanvasDecoration {
  NativeCanvasImage image;
  std::string anchor;
  double x = 0, y = 0, width = 0, height = 0;
  int z = 2;
  std::shared_ptr<const NativeCanvasAnimation> animation = {};
};
struct NativeCanvasBanner {
  NativeCanvasImage image;
  std::shared_ptr<const NativeCanvasAnimation> animation;
  double width = 0, height = 0;
  std::string align = "right";
  std::optional<msime::candidate::DecorationCandidate> candidate;
};

// 只消费共享扫描器的完整快照，不解释原始 TOML 颜色。
inline std::optional<msime::candidate::DecorationCandidate>
native_canvas_overlay(const nlohmann::json &object) {
  if (!object.is_object()) return {};
  msime::candidate::DecorationCandidate result;
  const auto number = [&](const char *key, double &destination) {
    const auto at = object.find(key);
    if (at == object.end() || !at->is_number()) return false;
    destination = at->get<double>();
    return std::isfinite(destination);
  };
  double index = -1;
  if (!number("index", index) || index < 0 || index > 100 || std::floor(index) != index ||
      !number("fontSizeDip", result.fontSizeDip) || !number("minFontSizeDip", result.minFontSizeDip) ||
      !number("maxWidthDip", result.maxWidthDip) || !number("offsetXDip", result.offsetXDip) ||
      !number("offsetYDip", result.offsetYDip)) return {};
  result.index = static_cast<int>(index);
  for (const auto &[key, destination] : {
      std::pair<const char *, std::string *>{"alignH", &result.alignH},
      {"alignV", &result.alignV}, {"resolvedColor", &result.color}}) {
    const auto at = object.find(key);
    if (at == object.end() || !at->is_string()) return {};
    *destination = at->get<std::string>();
  }
  const auto strip = object.find("stripBadge");
  if (strip == object.end() || !strip->is_boolean()) return {};
  result.stripBadge = strip->get<bool>();
  if (!result.valid() || result.color.size() != 9 || result.color.front() != '#' || !theme_color(result.color)) return {};
  return result;
}
struct NativeCanvasAssets {
  NativeCanvasImage panel;
  std::optional<NativeCanvasBanner> banner;
  std::array<double, 4> slice{}, width{}, content{12, 12, 12, 12}, outer{};
  bool fill = true;
  double line_gap = 0;
  std::vector<NativeCanvasDecoration> decorations;
  std::shared_ptr<const NativeCanvasAnimation> panel_animation = {};
  // 只保存共享 catalog 的已校验布局字段，按实际排列与字号延迟解析。
  nlohmann::json geometry = nlohmann::json::object();
  nlohmann::json horizontal = nlohmann::json::object(), vertical = nlohmann::json::object();
  std::string font_family;
  double min_width = 0;
  bool explicit_content = false, explicit_line_gap = false, fit_content = false;
  bool page_arrows = false, expand_panel = false;
  // 共享版式（resolvedLayout）的 expandRows，夹在 2–8：展开后一次看得见首行之后的几行（macOS 的 CandidateSkin::expandRows）。chrome 的网格和 canvas 包的卷轴都读它（native_canvas_expand_rows）；没写时为空，两种画法都按 kExpandedGridVisibleRows - 1 行。
  std::optional<std::size_t> expand_rows;
  double card_radius = 0, item_radius = 0, border_width = 0;
  bool user_radius = false;
  // candidate_opacity_percent：照 macOS 只淡化面板贴图、底色和描边，文字、序号和选中高亮不变；没有合成器时窗口不透明，半透明像素合到主题底色上，这个值不起作用。
  double opacity = 1;
  // true 时按 macOS 候选窗的结构画（NativeCanvasChrome.h）：首行是 logo、拼音、「1 / 3」、‹ › 和展开键，候选行按共享的 CandidateItemLayout 排，释义固定 12px。水杉自己的主题和非 canvas 皮肤包都走这条路；canvas 皮肤包仍按清单声明的 Windows 版式画（九宫格、翻页样式、辅助行在底部）。
  bool chrome = false;
  // candidate_scale_percent，乘到 chrome 的固定长度上（首行、行内边距、logo、释义字号）；canvas 包的长度来自清单，不再乘。
  double scale = 1;
  // 有选中条时序号前多让出 6pt（CandidateTextLeft）。
  bool show_selected_bar = false;
  // 非 canvas 皮肤包的顶带装饰图，照 macOS 的 DecorationPlacement 摆在卡片顶边上方的透明带里；top、width 已乘缩放。
  struct Band {
    std::shared_ptr<const NativeCanvasAnimation> image = {};
    double top = 0, width = 0;
    msime::candidate::DecorationAlign align = msime::candidate::DecorationAlign::right;
    msime::candidate::DecorationMotionKind motion = msime::candidate::DecorationMotionKind::none;
    double period_ms = 0;
    std::optional<msime::candidate::DecorationCandidate> candidate;
  } band;
  // 非 canvas 皮肤包的背景图（清单的 `[candidate_window.background]`），照 macOS 的 drawCandidateCard 画在卡片底色之上、描边与候选之下，裁在圆角里；动图取第一帧。`opacity` 是清单的不透明度，画的时候再乘候选窗不透明度。
  struct Background {
    NativeCanvasImage image;
    CandidateSkinBackgroundFit fit = CandidateSkinBackgroundFit::cover;
    double opacity = 1;
  } background;
  // 保留旧皮肤的动效字段；宿主始终采用静态候选呈现。
  msime::candidate::MotionPreset enter = msime::candidate::MotionPreset::none;
  msime::candidate::MotionPreset exit = msime::candidate::MotionPreset::none;
  msime::candidate::MotionPreset selection = msime::candidate::MotionPreset::none;
  double duration_ms = 0, stagger_ms = 0;
};

// 旧皮肤仍可加载，候选显隐与选中不执行过渡；图片帧与装饰待机由各自的通道播放。
inline void read_native_canvas_animation(NativeCanvasAssets &assets, const nlohmann::json &package) {
  (void)package;
  assets.enter = assets.exit = assets.selection = msime::candidate::MotionPreset::none;
  assets.duration_ms = assets.stagger_ms = 0;
}

inline NativeCanvasRect native_canvas_decoration_box(const NativeCanvasDecoration &decoration,
                                                     const NativeCanvasRect &panel) {
  const auto &anchor = decoration.anchor;
  const double x = anchor.find("left") != std::string::npos ? panel.x :
      anchor.find("right") != std::string::npos || anchor == "right" ? panel.x + panel.width - decoration.width :
      panel.x + (panel.width - decoration.width) / 2;
  const double y = anchor.find("top") != std::string::npos ? panel.y :
      anchor.find("bottom") != std::string::npos ? panel.y + panel.height - decoration.height :
      panel.y + (panel.height - decoration.height) / 2;
  return {x + decoration.x, y + decoration.y, decoration.width, decoration.height};
}

// 版式的 expandRows：整数才认，夹在 2–8；没写或类型不对时为空。canvas 包（load_native_canvas）和 chrome（native_canvas_theme_assets）共用。
inline std::optional<std::size_t> native_canvas_expand_rows(const nlohmann::json &layout) {
  const auto rows = layout.find("expandRows");
  if (rows == layout.end() || !rows->is_number_integer()) return std::nullopt;
  return static_cast<std::size_t>(std::clamp<std::int64_t>(rows->get<std::int64_t>(), 2, 8));
}

// 包已经经过共享 API 校验；这里仍复核相对路径，且每次换肤只解码一次。
inline std::shared_ptr<NativeCanvasAssets> load_native_canvas(const nlohmann::json &package,
                                                            const std::filesystem::path &directory,
                                                            const std::string &id, bool dark) {
  if (!package.is_object() || package.value("id", std::string{}) != id) return {};
  const bool canvas = package.value("canvas", false);
  const auto overlay = native_canvas_overlay(package.value("decorationCandidate", nlohmann::json{}));
  // 普通装饰图继续交给 classicui；只有动态图内候选词才需要原生绘制。
  if (!canvas && !overlay) return {};
  auto result = std::make_shared<NativeCanvasAssets>();
  // 解析结果已按出厂公共、出厂排列、包公共、包排列合并，不再叠加原始公共表。
  const auto resolved = package.find("resolvedLayout");
  const auto &layout = resolved != package.end() && resolved->is_object() ? *resolved : package;
  result->geometry = layout.value("geometry", nlohmann::json::object());
  result->horizontal = layout.value("horizontal", nlohmann::json::object());
  result->vertical = layout.value("vertical", nlohmann::json::object());
  result->font_family = package.value("fontFamily", std::string{});
  const auto number = [&](const char *key) {
    const auto at = layout.find(key);
    return at != layout.end() && at->is_number() ? at->get<double>() : 0.0;
  };
  result->card_radius = number("cornerRadiusDip");
  result->item_radius = number("itemCornerRadiusDip");
  result->border_width = number("borderWidthDip");
  result->min_width = package.value("minWidthDip", 0.0);
  result->explicit_content = canvas && package.contains("contentInsetsDip");
  result->explicit_line_gap = canvas && package.contains("lineGapDip");
  result->fit_content = layout.value("fitContent", false);
  result->page_arrows = layout.value("pageArrows", false);
  result->expand_panel = layout.value("expandPanel", false);
  result->expand_rows = native_canvas_expand_rows(layout);
  std::map<std::string, std::shared_ptr<const NativeCanvasAnimation>> cache;
  std::uint64_t remaining_pixels = 8000000;
  auto load = [&](const nlohmann::json &name) -> std::shared_ptr<const NativeCanvasAnimation> {
    const auto path = fcitx_canvas_resource(directory, id, name);
    if (!path) return {};
    const auto key = path->string();
    if (const auto at = cache.find(key); at != cache.end()) return at->second;
    const auto bytes = read_fcitx_canvas_image(*path);
    auto image = bytes ? decode_native_canvas_animation(*bytes, remaining_pixels) : nullptr;
    cache.emplace(key, image);
    return image;
  };
  if (const auto border = package.find("borderImage"); canvas && border != package.end() && border->is_object()) {
    result->panel_animation = load(border->at(!dark && border->contains("lightSource") ? "lightSource" : "source"));
    if (!result->panel_animation) return {};
    result->panel = result->panel_animation->frames.front();
    result->slice = fcitx_canvas_box(border->at(border->contains("stretchDip") ? "stretchDip" : "sliceDip")).value();
    result->width = border->contains("widthDip") ? fcitx_canvas_box(border->at("widthDip")).value() : result->slice;
    result->fill = border->value("fill", true);
  }
  for (std::size_t at = 0; at < 4; ++at) result->content[at] += result->width[at];
  if (result->explicit_content) {
    const auto &insets = package.at("contentInsetsDip");
    constexpr const char *keys[] = {"top", "right", "bottom", "left"};
    for (std::size_t at = 0; at < 4; ++at)
      result->content[at] = std::max(result->width[at], insets.at(keys[at]).get<double>());
  }
  if (canvas && package.contains("outerInsetsDip")) {
    const auto &insets = package.at("outerInsetsDip");
    result->outer = {insets.at("top"), insets.at("right"), insets.at("bottom"), insets.at("left")};
  }
  result->line_gap = canvas ? package.value("lineGapDip", 0.0) : 0;
  for (const auto &item : canvas ? package.value("decorations", nlohmann::json::array()) : nlohmann::json::array()) {
    auto image = load(item.at("image"));
    if (!image) return {};
    result->decorations.push_back({image->frames.front(), item.value("anchor", std::string("right")),
        item.value("offsetXDip", 0.0), item.value("offsetYDip", 0.0),
        item.at("widthDip"), item.at("heightDip"), item.value("zIndex", 2), std::move(image)});
  }
  std::stable_sort(result->decorations.begin(), result->decorations.end(),
      [](const auto &left, const auto &right) { return left.z < right.z; });
  const double width = package.value("decorationWidthDip", 0.0), height = package.value("decorationTopDip", 0.0);
  if (width > 0 && height > 0 && package.contains("decorationImage")) {
    auto animation = load(package.at("decorationImage"));
    if (animation) result->banner = NativeCanvasBanner{animation->frames.front(), animation, width, height,
        package.value("decorationAlign", std::string("right")), overlay};
  }
  if (!canvas && !result->banner) return {};
  read_native_canvas_animation(*result, package);
  return result;
}

struct NativeCanvasItem {
  std::string label, text;
  std::string translation = {}, suffix = {};
  nlohmann::json translation_runs = nlohmann::json::array();
  // 固定了排位的候选：未选中时正文画 macOS 的固定色 #379AD3（kCandidateFixedPositionColor），选中时照旧用选中文字色。
  bool fixed = false;
  // 以下只有 chrome 用：韩文汉字的训音画在释义行的第一行（macOS 的 glossReading），插件注释是释义行的最后一行、斜体；AI 占位行用次要色画。canvas 包仍把它们拼在 text 和 suffix 里。
  std::string reading = {};
  std::string script_annotation = {};
  bool pending = false;
  // 只有真实引擎候选携带装饰词；来源标注不混入原文。
  std::optional<std::string> decoration_text = {};
  std::string decoration_badge = {};
  // 光标后编辑里首选开头固定的那几个字：text 开头这么多字节用这一行序号的颜色（次要色）画淡，两种画法都认。0 表示不画淡。
  std::size_t dim_prefix_bytes = 0;
  // 「仅当前高亮」的候选词翻译生效时，高亮这一行（视图的 translation_reserved）一直留出的释义行数，回答还没到也留，行数与横排为未到释义预留的相同；移动高亮时卡片高度不跳。chrome 的横排由 NativeCanvasContent::reserved_gloss_lines 统一预留，不看这个；chrome 的竖排和 canvas 包按有没有释义变高，都按它留。
  int reserved_gloss_lines = 0;
  // 释义各行的读音，"\n" 分行、与 translation 逐行对应（CandidatePronunciation.h）；两种画法都把它画在它那一行释义下面单独一行，按释义色减淡。没有打开「显示读音」或没有音标时为空。
  std::string pronunciation = {};
};
struct NativeCanvasAction { std::string text; std::function<void()> invoke; };
struct NativeCanvasContent {
  std::string preedit, auxiliary;
  std::string font = "sans 16";
  bool horizontal = false;
  int selected = -1;
  // UTF-8 字节位置；-1 表示没有与显示文字同源的光标。
  int preedit_cursor = -1;
  bool menu = false;
  bool previous = false, next = false, wheel_paging = false;
  // show_candidate_page_number：关掉时不画 ‹ ›，与 macOS 一起隐藏页码和箭头相同；滚轮和翻页键照常。
  bool page_arrows = true;
  std::vector<NativeCanvasItem> items;
  // 展开网格仍从普通当前页取装饰词，不能随网格的可见行与游标变化。
  std::optional<std::vector<NativeCanvasItem>> decoration_items;
  int decoration_selected = -1;
  // 回调持有原 CandidateList 快照；身份围栏仍由 CandidateWord 与运行时复核。
  std::function<void(int)> select;
  std::function<void(bool)> page;
  std::function<std::vector<NativeCanvasAction>(int)> actions;
  // 展开候选，两种画法共用：expandable 表示提供展开／收起键，expand(true) 展开、expand(false) 收起；expanded 表示已经展开（chrome 的网格或 canvas 包的卷轴）。展开时滚轮滚动内容而不翻页，不看「滚轮翻页」开关。宿主那边是同一份展开状态（FcitxEngine 的 expanded_），完整候选、按全局身份选词和代次核对都在那里。
  bool expandable = false, expanded = false;
  std::function<void(bool)> expand;
  // 以下只有 chrome 用。拼音的字号（已乘缩放的 candidate_preedit_font_size，0 跟随候选字号）；首行打头的 logo；当前页和总页数（「1 / 3」）。
  double preedit_size = 0;
  // 底部一行右端的提示（光标后编辑的「候选编辑：光标后」），和左边的 auxiliary 同一行、同一种颜色，不能点。canvas 包没有底部一行，提示在 auxiliary 里跟着辅助行走。
  std::string auxiliary_hint;
  NativeCanvasImage logo = {};
  int page_index = 0, page_count = 0;
  // 横排为还没到的释义预留几行（macOS 的 reservedGlossHeightForFont），释义到达时卡片不在用户眼前变高。
  int reserved_gloss_lines = 0;
  // 横排卡片这次显示以来最宽的一次（WidestCandidateWidth），卡片只变宽不变窄。
  double minimum_width = 0;
  // 展开的候选网格：grid_columns 是每行几个（每页候选数），items 是可见的那几行，按行排；selected 是高亮在 items 里的位置。首行显示「grid_row + 1 / grid_rows」和收起键，滚轮经 grid_scroll 滚动一行。
  std::size_t grid_columns = 0, grid_row = 0, grid_rows = 0;
  std::function<void(bool)> grid_scroll;
  // 这一页属于哪一段组字（编辑文字加页码）：高亮只在同一段组字、同一页里换行时才播选中动效。
  std::string selection_context;
  // 以下只有 canvas 包的卷轴用（Windows 的 expandPanel）：items 仍是当前页，grid_items 是完整列表里当前页之后的候选，命中下标接在 items 后面，select 一并处理。revision 相同的重画保留滚动位置，换代、翻页或简繁投影变化时回到顶部；grid_limit 是已经排版的网格项数，滚到接近底部时再加一批。
  std::string revision;
  std::size_t page_size = 9, grid_limit = 300;
  double scroll_offset = 0;
  std::vector<NativeCanvasItem> grid_items;
};
struct NativeCanvasPointer { int hovered = -1, pressed = -1; };

// 命中测试的结果：0 起是候选，下面这些是首行和翻页的控件，-1 什么也不是。
constexpr int kNativeCanvasHitPrevious = -2;
constexpr int kNativeCanvasHitNext = -3;
constexpr int kNativeCanvasHitExpand = -4;
// canvas 包卷轴的滚动条（轨道和滑块）。
constexpr int kNativeCanvasHitScrollbar = -5;

// 只消费共享层规范化后的颜色，保留用户取色器与可读选中文字的优先级。
// 保留主题明确声明的槽位，不能把 canvas 回退色覆盖到 chrome 的系统调色板上。
inline nlohmann::json native_canvas_declared_palette(const nlohmann::json &document) {
  auto palette = document.value("candidate", nlohmann::json{});
  if (!palette.is_object()) palette = nlohmann::json::object();
  if (const auto details = document.find("candidate_details"); details != document.end() && details->is_object())
    for (const auto &[key, value] : details->items()) if (!value.is_null()) palette[key] = value;
  return palette;
}

inline nlohmann::json native_canvas_palette(const nlohmann::json &document, bool dark = false) {
  auto palette = native_canvas_declared_palette(document);
  const auto colors = candidate_theme_colors(document, dark).colors;
  for (const auto &[key, color] : {
      std::pair<const char *, std::optional<std::uint32_t>>{"surface", colors.background},
      {"text", colors.text}, {"number", colors.number}, {"secondary", colors.number}, {"accent", colors.accent},
      {"selected", colors.selected}, {"selected_text", colors.selected_text},
      {"selected_number", colors.selected_number}, {"border", colors.border}})
    if (color && !theme_color(palette.value(key, nlohmann::json{}))) palette[key] = fcitx_theme_color(*color);
  return palette;
}

struct NativeCanvasMetrics {
  std::string font, number_font;
  double font_size = 0, number_scale = 1, number_min_width = 0;
  std::array<double, 4> content{}, preedit_margin{};
  double gap = 4, extra_height = 8, number_inset = 8, right = 8, number_gap = 0, min_item_width = 0;
  bool custom_height = false, justify = false, corners_follow_card = false, bar_inside = false;
  bool triangle_pager = false;
  double card_radius = 0, item_radius = 0, bar_width = 3, bar_height = 0;
};

inline NativeCanvasMetrics native_canvas_metrics(cairo_t *cr, const NativeCanvasAssets &assets,
                                                  const NativeCanvasContent &content) {
  NativeCanvasMetrics result;
  auto geometry = assets.geometry;
  geometry.update(content.horizontal ? assets.horizontal : assets.vertical);
  auto *font = pango_font_description_from_string(content.font.c_str());
  if (!assets.font_family.empty()) {
    const char *configured = pango_font_description_get_family(font);
    const auto family = assets.font_family + (configured ? "," + std::string(configured) : "");
    pango_font_description_set_family(font, family.c_str());
  }
  // Pango 的普通字号是 point；布局和皮肤 em 使用 DIP，显示缩放随后由 Cairo 统一施加。
  auto *context = pango_cairo_create_context(cr);
  double dpi = pango_cairo_context_get_resolution(context);
  if (dpi <= 0) dpi = pango_cairo_font_map_get_resolution(PANGO_CAIRO_FONT_MAP(pango_context_get_font_map(context)));
  result.font_size = pango_font_description_get_size(font) / static_cast<double>(PANGO_SCALE);
  if (!pango_font_description_get_size_is_absolute(font)) result.font_size *= dpi / 72.0;
  g_object_unref(context);
  auto string = [](const PangoFontDescription *description) {
    char *value = pango_font_description_to_string(description);
    std::string result(value);
    g_free(value);
    return result;
  };
  result.font = string(font);
  result.number_scale = geometry.value("numberScale", 1.0);
  if (geometry.contains("numberScale") || geometry.contains("numberInset") || geometry.contains("numberGap"))
    result.number_min_width = 9;
  // 0 表示隐藏序号；避免构造 0 字号被 Pango 回退为默认字号。
  pango_font_description_set_absolute_size(font, std::max(0.01, result.font_size * result.number_scale) * PANGO_SCALE);
  result.number_font = string(font);
  pango_font_description_free(font);
  const auto length = [&](const char *key, double fallback) {
    if (!geometry.contains(key)) return fallback;
    const auto &value = geometry.at(key);
    const double amount = value.at("value");
    return value.value("em", false) ? std::round(amount * result.font_size) : amount;
  };
  result.content = assets.content;
  if (geometry.contains("padding") && !assets.explicit_content) {
    const double padding = length("padding", 12);
    for (std::size_t at = 0; at < 4; ++at) result.content[at] = assets.width[at] + padding;
  }
  result.card_radius = assets.card_radius;
  result.item_radius = assets.item_radius;
  result.corners_follow_card = assets.user_radius || geometry.value("itemCornersFollowCard", false);
  result.bar_width = std::max(3.0, length("selectedBarWidth", 3));
  result.bar_height = std::max(result.bar_width * 2, result.font_size * geometry.value("selectedBarHeightEm", 0.85));
  result.bar_inside = geometry.value("selectedBarInside", false);
  result.gap = length("itemGap", 4);
  if (content.horizontal && geometry.contains("itemGap") && !assets.fit_content) result.gap += 6;
  result.custom_height = geometry.contains("itemExtraHeight");
  result.extra_height = length("itemExtraHeight", 8);
  // 来源把序号左缘限制在至少 5 DIP；未指定时保留原生窗口的 8 DIP。
  result.number_inset = geometry.contains("numberInset") ? std::max(5.0, length("numberInset", 8)) : 8;
  result.right = length("itemPaddingRight", 8);
  if (assets.fit_content) result.right = result.number_inset;
  result.number_gap = length("numberGap", 0);
  result.min_item_width = content.horizontal ? length("itemMinWidth", 0) : 0;
  result.justify = content.horizontal && geometry.value("justifyRows", false);
  result.triangle_pager = geometry.value("pageArrowStyle", std::string("chevron")) == "triangle";
  if (geometry.contains("preeditMarginDip")) {
    const auto &margin = geometry.at("preeditMarginDip");
    result.preedit_margin = {margin.at("top"), margin.at("right"), margin.at("bottom"), margin.at("left")};
  }
  if (assets.explicit_line_gap || assets.line_gap != 0) result.preedit_margin[2] = assets.line_gap;
  return result;
}

struct NativeCanvasPager {
  bool visible = false, triangle = false;
  double button_width = 0, button_height = 0, gap = 1, glyph = 0, stroke = 0, radius = 0;
  double divider_width = 0, divider_gap = 0, left = 0, right = 0, lift = 0;
  double glyph_width() const { return glyph * (triangle ? 0.84 : 0.5 * 0.55); }
  double width() const { return divider_width + divider_gap + 2 * button_width + gap; }
  double extent() const { return left + width() + right; }
};

inline NativeCanvasPager native_canvas_pager(const NativeCanvasAssets &assets,
    const NativeCanvasContent &content, const NativeCanvasMetrics &metrics, double item_height) {
  NativeCanvasPager pager;
  // 横排提供展开键时它代替翻页标记；释义页这类不提供展开的横排仍画翻页。
  pager.visible = assets.page_arrows && content.page_arrows && !(content.horizontal && assets.expand_panel && content.expandable) &&
      !content.menu && !content.items.empty();
  if (!pager.visible) return pager;
  pager.triangle = metrics.triangle_pager;
  const double em = metrics.font_size;
  pager.button_width = std::round(em * 0.72);
  pager.button_height = std::round(em * 0.9);
  pager.glyph = em * 0.5;
  pager.stroke = std::max(1.2, em / 13);
  pager.radius = std::min(3.0, metrics.item_radius);
  if (pager.triangle) {
    pager.glyph = em * 0.52;
    pager.stroke = pager.glyph * 0.375;
    pager.radius = metrics.item_radius;
    pager.gap = 0;
    pager.button_width = std::round(em * (content.horizontal ? 0.96 : 0.81));
    pager.button_height = content.horizontal ? std::round(item_height) : std::round(em * 1.2);
    if (content.horizontal) {
      pager.divider_width = 1;
      pager.divider_gap = std::round(em * 0.12);
      pager.left = std::round(em * 0.3);
    }
  } else if (content.horizontal) {
    pager.left = 2; pager.right = 1;
    pager.lift = std::max(0.0, (item_height - pager.button_height) / 2);
  }
  return pager;
}

struct NativeCanvasLayout {
  NativeCanvasMetrics metrics;
  NativeCanvasPager pager;
  NativeCanvasRect panel, preedit, auxiliary, previous, next, page_divider;
  std::vector<NativeCanvasRect> items;
  // 保留测量时的文字宽度；均分背景后绘制仍沿用原断行。
  std::vector<double> item_text_widths;
  double preedit_width = -1, auxiliary_width = -1;
  double width = 0, height = 0;
  struct Overlay {
    std::string text;
    double font_size = 0;
    NativeCanvasRect box, ink;
  };
  std::optional<Overlay> overlay;
  // 展开／收起键：chrome 在首行或第一行候选的右端，canvas 包固定在横排右侧；命中、绘制和 X11 输入形状共用它。
  NativeCanvasRect expand;
  // 以下只有 canvas 包的卷轴填写：首行和网格一起在 viewport 里滚动，滚出去的部分既不画也不命中；scrollbar 是轨道，thumb 是滑块。
  NativeCanvasRect viewport, scrollbar, thumb;
  std::vector<NativeCanvasRect> grid;
  std::vector<double> grid_text_widths;
  double scroll_offset = 0, max_scroll = 0, scroll_step = 0;
  double grid_label_width = 0;
  // 以下只有 chrome 布局填写（NativeCanvasChrome.h）。auxiliary_hint 是底部一行右端那段提示的位置。
  bool chrome = false;
  NativeCanvasRect logo, indicator, band, auxiliary_hint;
  std::string indicator_text;
  bool page_controls = false;
  std::string gloss_font, preedit_font, indicator_font, symbol_font;
  // 行内的列：序号左缘、右边距和每行正文的左缘（都相对行的左边；网格里只有高亮那一行有序号，各行正文左缘不同），以及共享排版给每行的文字、释义位置。皮肤带版式（skin_layout）时选中条骑在行的左缘上。
  double text_left = 0, text_right = 0;
  std::vector<double> content_lefts;
  bool skin_layout = false;
  std::vector<msime::candidate::CandidateItemLayout> rows;
  // 拼音一行画不下时向左滚，让插入光标留在框里。
  double preedit_scroll = 0;
  // 候选行所在的区域；贴卡片圆角的高亮按它判断哪几行在边上。
  NativeCanvasRect rows_area;
};

inline PangoLayout *native_canvas_text_layout(cairo_t *cr, const std::string &font,
    const std::string &text, double max_width = -1, double font_size = 0) {
  auto *layout = pango_cairo_create_layout(cr);
  auto *description = pango_font_description_from_string(font.c_str());
  if (font_size > 0) pango_font_description_set_absolute_size(description, font_size * PANGO_SCALE);
  pango_layout_set_font_description(layout, description);
  pango_layout_set_text(layout, text.c_str(), static_cast<int>(text.size()));
  if (std::isfinite(max_width) && max_width >= 0) {
    pango_layout_set_width(layout, static_cast<int>(std::clamp(std::floor(max_width * PANGO_SCALE),
        1.0, static_cast<double>(G_MAXINT))));
    pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
  }
  pango_font_description_free(description);
  return layout;
}

inline std::pair<double, double> native_canvas_text_size(cairo_t *cr, const std::string &font,
    const std::string &text, double max_width = -1, double font_size = 0) {
  auto *layout = native_canvas_text_layout(cr, font, text, max_width, font_size);
  int width = 0, height = 0;
  pango_layout_get_pixel_size(layout, &width, &height);
  g_object_unref(layout);
  return {static_cast<double>(width), static_cast<double>(height)};
}

inline NativeCanvasRect native_canvas_banner_box(const NativeCanvasBanner &banner, const NativeCanvasRect &panel) {
  const double align = banner.align == "left" ? 0 : banner.align == "center" ? 0.5 : 1;
  return {panel.x + (panel.width - banner.width) * align, panel.y - banner.height, banner.width, banner.height};
}

inline PangoLayout *native_canvas_overlay_line(cairo_t *cr, const std::string &font,
    const std::string &text, double size) {
  auto *line = native_canvas_text_layout(cr, font, text, -1, size);
  pango_layout_set_single_paragraph_mode(line, true);
  return line;
}

inline std::optional<NativeCanvasLayout::Overlay> native_canvas_overlay_layout(cairo_t *cr,
    const msime::candidate::DecorationCandidate &style, const NativeCanvasContent &content,
    const NativeCanvasLayout &layout, const NativeCanvasRect &box) {
  if (content.menu) return {};
  const auto &items = content.decoration_items ? *content.decoration_items : content.items;
  const int selected = content.decoration_items ? content.decoration_selected : content.selected;
  std::vector<const NativeCanvasItem *> words;
  int highlight = -1;
  for (std::size_t at = 0; at < items.size(); ++at) {
    const auto &item = items[at];
    if (!item.decoration_text) continue;
    if (static_cast<int>(at) == selected) highlight = static_cast<int>(words.size());
    words.push_back(&item);
  }
  const auto seat = style.seat(words.size(), highlight);
  if (!seat) return {};
  const auto &item = *words[*seat];
  const auto text = *item.decoration_text + (style.stripBadge ? "" : item.decoration_badge);
  if (text.empty()) return {};
  auto *line = native_canvas_overlay_line(cr, layout.metrics.font, text, style.fontSizeDip);
  int width = 0, height = 0;
  pango_layout_get_pixel_size(line, &width, &height);
  const double size = style.fittedFont(width, box.width);
  if (size != style.fontSizeDip) {
    g_object_unref(line);
    line = native_canvas_overlay_line(cr, layout.metrics.font, text, size);
    pango_layout_get_pixel_size(line, &width, &height);
  }
  PangoRectangle ink{};
  pango_layout_get_pixel_extents(line, &ink, nullptr);
  g_object_unref(line);
  const double x = box.x + style.left(box.width, width), y = box.y + style.top(box.height, height);
  return NativeCanvasLayout::Overlay{text, size, {x, y, static_cast<double>(width), static_cast<double>(height)},
      {x + ink.x, y + ink.y, static_cast<double>(ink.width), static_cast<double>(ink.height)}};
}

// 释义范围按原串字节解释；只给共享层识别的区段加属性，不在宿主重做词性识别。
inline PangoLayout *native_canvas_item_layout(cairo_t *cr, const NativeCanvasMetrics &metrics,
    const NativeCanvasItem &item, bool horizontal, const nlohmann::json &colors = nlohmann::json::object(),
    bool highlighted = false, double max_width = -1) {
  // 读音单独占一行，有读音时释义总在正文下面，不和正文挤在同一行。
  const auto glossed = candidate_gloss_readings(item.translation, item.pronunciation);
  bool inline_translation = !horizontal && glossed.readings.empty();
  if (inline_translation && !item.translation.empty() && std::isfinite(max_width) && max_width >= 0) {
    auto *body = native_canvas_text_layout(cr, metrics.font, item.text);
    int width = 0, height = 0;
    pango_layout_get_pixel_size(body, &width, &height);
    const auto translation = native_canvas_text_size(cr, metrics.font, item.translation, -1, metrics.font_size * 0.78);
    inline_translation = pango_layout_get_line_count(body) == 1 &&
        width + metrics.font_size * 0.65 + translation.first <= max_width;
    g_object_unref(body);
  }
  const std::string gap = item.translation.empty() ? "" : inline_translation ? " " : "\n";
  const auto value = item.text + gap + glossed.text + item.suffix;
  auto *line = native_canvas_text_layout(cr, metrics.font, value, max_width);
  const auto dim_end = std::min(item.dim_prefix_bytes, item.text.size());
  if (item.translation.empty() && dim_end == 0) return line;
  const auto start = item.text.size() + gap.size(), end = start + glossed.text.size();
  auto *attributes = pango_attr_list_new();
  const auto insert = [&](PangoAttribute *attribute, std::size_t from, std::size_t to) {
    attribute->start_index = static_cast<guint>(from);
    attribute->end_index = static_cast<guint>(to);
    pango_attr_list_change(attributes, attribute);
  };
  const auto color = [&](const char *key) { return theme_color(colors.value(key, nlohmann::json{})); };
  const auto paint = [&](ThemeColor color, std::size_t from, std::size_t to) {
    insert(pango_attr_foreground_new(((color.rgb >> 16) & 255) * 257,
        ((color.rgb >> 8) & 255) * 257, (color.rgb & 255) * 257), from, to);
    insert(pango_attr_foreground_alpha_new(color.alpha * 257), from, to);
  };
  // 光标后编辑：首选开头固定的那几个字用这一行序号的颜色画淡，与序号的取色一致（选中行取选中序号色，主题没给时是 #555555）。只改颜色不改字形，测量时（colors 为空）同样带着这段属性，宽度不变。
  if (dim_end > 0) {
    auto dim = highlighted ? color("selected_number") : std::optional<ThemeColor>{};
    if (!dim) dim = color("number");
    paint(dim.value_or(ThemeColor{0x555555, 255}), 0, dim_end);
  }
  if (item.translation.empty()) {
    pango_layout_set_attributes(line, attributes);
    pango_attr_list_unref(attributes);
    return line;
  }
  insert(pango_attr_size_new_absolute(static_cast<int>(std::round(metrics.font_size * 0.78 * PANGO_SCALE))), start, end);
  if (inline_translation) {
    PangoRectangle ink{}, logical{0, 0, static_cast<int>(std::round(metrics.font_size * 0.65 * PANGO_SCALE)), 0};
    insert(pango_attr_shape_new(&ink, &logical), item.text.size(), start);
  }
  const auto visible = [](const std::optional<ThemeColor> &color) { return color && color->alpha > 0; };
  auto base = color(highlighted ? "selected_text" : "text");
  if (!base) base = color("text");
  if (!base) base = ThemeColor{0x202020, 255};
  auto translation = highlighted ? color("selected_translation") : std::optional<ThemeColor>{};
  if (!visible(translation)) translation = color("translation");
  if (!visible(translation)) {
    translation = base;
    translation->alpha = static_cast<std::uint8_t>(std::round(translation->alpha * 0.62));
  }
  paint(*translation, start, end);
  if (item.translation_runs.is_array()) for (const auto &run : item.translation_runs) {
    if (!run.is_object() || !run.contains("start") || !run.contains("length") ||
        !run["start"].is_number_integer() || !run["length"].is_number_integer() ||
        run["start"] < 0 || run["length"] <= 0 || run["start"] > item.translation.size() ||
        !run.contains("role") || !run["role"].is_string()) continue;
    const auto from = run["start"].get<std::size_t>(), length = run["length"].get<std::size_t>();
    if (length > item.translation.size() - from ||
        !g_utf8_validate(item.translation.data() + from, static_cast<gssize>(length), nullptr)) continue;
    const auto role = run["role"].get<std::string>();
    if (role != "pos" && role != "separator") continue;
    const char *normal = role == "pos" ? "translation_pos" : "translation_separator";
    const char *selected = role == "pos" ? "selected_translation_pos" : "selected_translation_separator";
    auto run_color = highlighted ? color(selected) : std::optional<ThemeColor>{};
    if (!visible(run_color)) run_color = color(normal);
    const auto at = start + glossed.position(from);
    paint(visible(run_color) ? *run_color : *translation, at, at + length);
  }
  // 读音比它跟着的释义轻一些：释义色的 0.72 倍透明度（macOS 的 MSIMECandidatePronunciationOpacity）；选中行不减淡，那时的释义色已经是选中色。
  if (!glossed.readings.empty()) {
    auto reading = *translation;
    if (!highlighted)
      reading.alpha = static_cast<std::uint8_t>(std::round(reading.alpha * msime::candidate::kCandidatePronunciationOpacity));
    for (const auto &[from, length] : glossed.readings) paint(reading, start + from, start + from + length);
  }
  pango_layout_set_attributes(line, attributes);
  pango_attr_list_unref(attributes);
  return line;
}

inline std::pair<double, double> native_canvas_item_size(cairo_t *cr, const NativeCanvasMetrics &metrics,
    const NativeCanvasItem &item, bool horizontal, double max_width = -1) {
  auto *line = native_canvas_item_layout(cr, metrics, item, horizontal, nlohmann::json::object(), false, max_width);
  int width = 0, height = 0;
  pango_layout_get_pixel_size(line, &width, &height);
  g_object_unref(line);
  return {static_cast<double>(width), static_cast<double>(height)};
}

inline NativeCanvasLayout measure_native_canvas_skin(cairo_t *cr, const NativeCanvasAssets &assets,
    const NativeCanvasContent &content, double panel_width_limit = -1, double panel_height_limit = -1) {
  NativeCanvasLayout layout;
  layout.metrics = native_canvas_metrics(cr, assets, content);
  const auto &metrics = layout.metrics;
  const auto &padding = metrics.content;
  const double available = panel_width_limit > 0 ? std::max(1.0, panel_width_limit - padding[3] - padding[1]) :
      std::numeric_limits<double>::infinity();
  const auto line = native_canvas_text_size(cr, metrics.font, "国Mg").second;
  const auto number_line = metrics.number_scale == 0 ? 0 : native_canvas_text_size(cr, metrics.number_font, "Mg").second;
  double y = padding[0], width = 0;
  if (!content.preedit.empty()) {
    const auto &margin = metrics.preedit_margin;
    const double caret = std::isfinite(available) && content.preedit_cursor >= 0 && !content.menu ? 2 : 0;
    layout.preedit_width = std::isfinite(available) ? std::max(1.0, available - margin[3] - margin[1] - caret) : -1;
    const auto size = native_canvas_text_size(cr, metrics.font, content.preedit, layout.preedit_width);
    const double height = std::max(line, size.second);
    layout.preedit = {padding[3] + margin[3], y + margin[0], size.first, height};
    width = size.first + margin[3] + margin[1] + caret;
    y += margin[0] + height + margin[2];
  }
  std::pair<double, double> auxiliary{};
  if (!content.auxiliary.empty()) {
    layout.auxiliary_width = std::isfinite(available) ? available : -1;
    auxiliary = native_canvas_text_size(cr, metrics.font, content.auxiliary, layout.auxiliary_width);
    auxiliary.second = std::max(line, auxiliary.second);
    width = std::max(width, auxiliary.first);
  }
  width = std::max(width, std::min(available, std::max(0.0, assets.min_width - padding[3] - padding[1])));
  layout.pager = native_canvas_pager(assets, content, metrics, metrics.font_size * 1.35 + metrics.extra_height);
  const auto &pager = layout.pager;
  // 卷轴：清单打开 expandPanel、横排、宿主提供展开键时，右侧固定一个展开／收起键；展开后首行之下排完整列表的后续候选。
  const bool expand = assets.expand_panel && content.horizontal && !content.menu &&
      !content.items.empty() && content.expandable;
  const bool grid = expand && content.expanded && !content.grid_items.empty();
  const double button = std::round(metrics.font_size * 1.1), toggle_extent = expand ? button + 3 : 0;
  // 横排翻页独占右侧一列，候选只在其左侧折行。
  const double reserved = pager.visible && content.horizontal ? pager.extent() : toggle_extent;
  const double list_available = std::max(1.0, available - reserved - (grid ? 10 : 0));
  std::vector<double> natural_widths, text_offsets;
  double vertical_width = width;
  for (const auto &item : content.items) {
    const auto size = native_canvas_item_size(cr, metrics, item, content.horizontal);
    const auto label = item.label.empty() ? 0 : std::max(metrics.number_min_width,
        metrics.number_scale == 0 ? 0 : native_canvas_text_size(cr, metrics.number_font, item.label).first);
    const double offset = metrics.number_inset + label + (label > 0 ? metrics.number_gap : 0);
    text_offsets.push_back(offset);
    const double natural = std::max(metrics.min_item_width, size.first + offset + metrics.right);
    natural_widths.push_back(natural);
    vertical_width = std::max(vertical_width, natural);
  }
  vertical_width = std::min(vertical_width, available);
  const double row_y = y;
  const double base_height = std::max({line, number_line, metrics.font_size * 1.35 + metrics.extra_height});
  double x = padding[3], row_height = 0, list_width = 0;
  std::size_t first = 0;
  std::vector<std::pair<std::size_t, std::size_t>> rows;
  const auto finish_row = [&] {
    for (std::size_t at = first; at < layout.items.size(); ++at) layout.items[at].height = row_height;
    rows.emplace_back(first, layout.items.size());
  };
  for (std::size_t at = 0; at < content.items.size(); ++at) {
    const auto &item = content.items[at];
    const double fixed = text_offsets[at] + metrics.right;
    double item_width = std::max(fixed + 1, content.horizontal ? std::min(natural_widths[at], list_available) : vertical_width);
    const double text_width = std::isfinite(available) ? std::max(1.0, item_width - fixed) : -1;
    const auto size = native_canvas_item_size(cr, metrics, item, content.horizontal, text_width);
    // 单个不可拆字形或序号栏也放不下时，让窗口尺寸复核触发原有回退，不能悄悄裁字。
    item_width = std::max(item_width, size.first + fixed);
    double item_height = metrics.custom_height ? std::max(base_height, size.second) :
        std::max({line, number_line, size.second}) + metrics.extra_height;
    // 「仅当前高亮」的高亮行一直留出释义的行数：canvas 包按有没有释义变高、没有统一预留，竖排横排都按 reserved_gloss_lines 量一份释义放在正文下面时的高度，回答没到也留。
    if (item.reserved_gloss_lines > 0) {
      auto reserved = item;
      reserved.pronunciation.clear();
      reserved.translation = "X";
      for (int extra = 1; extra < item.reserved_gloss_lines; ++extra) reserved.translation += "\nX";
      reserved.translation_runs = nlohmann::json::array();
      const auto placeholder = native_canvas_item_size(cr, metrics, reserved, true, text_width);
      item_height = std::max(item_height, metrics.custom_height ? std::max(base_height, placeholder.second) :
          std::max({line, number_line, placeholder.second}) + metrics.extra_height);
    }
    if (content.horizontal && x > padding[3] && x + item_width > padding[3] + list_available + 0.001) {
      finish_row();
      y += row_height + metrics.gap;
      x = padding[3]; row_height = 0; first = layout.items.size();
    }
    layout.items.push_back({x, y, item_width, item_height});
    layout.item_text_widths.push_back(text_width);
    list_width = std::max(list_width, x - padding[3] + item_width);
    width = std::max(width, x - padding[3] + item_width);
    row_height = std::max(row_height, item_height);
    if (content.horizontal) x += item_width + metrics.gap;
    else y += item_height + metrics.gap;
  }
  if (content.horizontal && !layout.items.empty()) {
    finish_row();
    y += row_height;
  }
  if (!content.horizontal && !layout.items.empty()) y -= metrics.gap;
  const double list_bottom = y;
  if (expand) {
    width = std::max(width, list_width + toggle_extent + (grid ? 10 : 0));
    if (grid && std::isfinite(available)) width = std::max(width, available);
    const double right = padding[3] + width - toggle_extent;
    layout.expand = {right + 2, row_y + std::max(0.0, (row_height - button) / 2), button, button};
    y = std::max(y, layout.expand.y + button);
    const double first_height = y - row_y;
    layout.scroll_step = base_height + metrics.gap;
    if (grid) {
      layout.grid_label_width = text_offsets.empty() ? metrics.number_min_width + metrics.number_gap :
          text_offsets.front() - metrics.number_inset;
      const double grid_width = std::max(1.0, width - toggle_extent - 10);
      const auto columns = static_cast<std::size_t>(std::clamp(std::floor(
          (grid_width + metrics.gap) / (metrics.font_size * 2.6 + metrics.gap)),
          1.0, static_cast<double>(std::max(std::size_t{1}, content.page_size))));
      const double cell = std::max(1.0, (grid_width - (columns - 1) * metrics.gap) / columns);
      double gx = 0, gy = y + metrics.gap, gh = base_height;
      std::size_t begin = 0;
      const auto finish = [&] {
        for (std::size_t at = begin; at < layout.grid.size(); ++at) layout.grid[at].height = gh;
      };
      for (std::size_t at = 0; at < std::min(content.grid_limit, content.grid_items.size()); ++at) {
        const auto &item = content.grid_items[at];
        const auto natural = native_canvas_item_size(cr, metrics, item, true);
        const double fixed = metrics.number_inset + layout.grid_label_width + metrics.right;
        const double span = std::clamp(std::ceil((natural.first + fixed + metrics.gap) /
            (cell + metrics.gap) - 0.001), 1.0, static_cast<double>(columns));
        const double cell_width = std::min(grid_width, span * cell + (span - 1) * metrics.gap);
        if (gx > 0 && gx + cell_width > grid_width + 0.5) {
          finish(); gy += gh + metrics.gap; gx = 0; gh = base_height; begin = layout.grid.size();
        }
        const double text_width = std::max(1.0, cell_width - fixed);
        const auto size = native_canvas_item_size(cr, metrics, item, true, text_width);
        gh = std::max(gh, size.second + (metrics.custom_height ? 0 : metrics.extra_height));
        layout.grid.push_back({padding[3] + gx, gy, cell_width, gh});
        layout.grid_text_widths.push_back(text_width);
        gx += cell_width + metrics.gap;
      }
      finish();
      const double bottom = gy + gh;
      const auto extra_rows = static_cast<double>(assets.expand_rows.value_or(msime::candidate::kExpandedGridVisibleRows - 1));
      double viewport = std::min(bottom - row_y, first_height + extra_rows * layout.scroll_step);
      if (panel_height_limit > 0) viewport = std::min(viewport, std::max(first_height,
          panel_height_limit - row_y - padding[2] - (content.auxiliary.empty() ? 0 : auxiliary.second + 4)));
      layout.viewport = {padding[3], row_y, width - toggle_extent, viewport};
      layout.max_scroll = std::max(0.0, bottom - row_y - viewport);
      layout.scroll_offset = std::clamp(content.scroll_offset, 0.0, layout.max_scroll);
      if (layout.max_scroll > 0) {
        layout.scrollbar = {right - 10, row_y, 10, viewport};
        const double height = std::min(viewport, std::max(20.0, viewport * viewport / (bottom - row_y)));
        layout.thumb = {right - 6, row_y + (viewport - height) * layout.scroll_offset / layout.max_scroll, 4, height};
      }
      for (auto &box : layout.items) box.y -= layout.scroll_offset;
      for (auto &box : layout.grid) box.y -= layout.scroll_offset;
      y = row_y + viewport;
    }
  }
  if (pager.visible) {
    if (content.horizontal) {
      width = std::max(width, list_width + pager.extent());
      y = std::max(y, row_y + pager.button_height + pager.lift);
    } else {
      const double left = std::max(0.0, padding[3] + metrics.number_inset -
          (pager.button_width - pager.glyph_width()) / 2);
      layout.previous = {left, y + 1, pager.button_width, pager.button_height};
      layout.next = {left + pager.button_width + pager.gap, y + 1, pager.button_width, pager.button_height};
      width = std::max(width, left + pager.width() - padding[3]);
      y += pager.button_height + 2;
    }
  }
  if (!content.auxiliary.empty()) {
    y += 4;
    layout.auxiliary = {padding[3], y, auxiliary.first, auxiliary.second};
    y += auxiliary.second;
  }
  if (metrics.justify) for (const auto &[begin, end] : rows) {
    const double used = layout.items[end - 1].x + layout.items[end - 1].width - padding[3];
    const double extra = std::max(0.0, width - reserved - (grid ? 10 : 0) - used) / (end - begin);
    for (std::size_t at = begin; at < end; ++at) {
      layout.items[at].x += (at - begin) * extra;
      layout.items[at].width += extra;
    }
  }
  if (pager.visible && content.horizontal) {
    const double right = padding[3] + width - pager.right;
    const double bottom = std::max(list_bottom, row_y + pager.button_height + pager.lift) - pager.lift;
    layout.previous = {right - 2 * pager.button_width - pager.gap, bottom - pager.button_height,
        pager.button_width, pager.button_height};
    layout.next = {right - pager.button_width, layout.previous.y, pager.button_width, pager.button_height};
    layout.page_divider = {layout.previous.x - pager.divider_gap - pager.divider_width, layout.previous.y,
        pager.divider_width, pager.button_height};
  }
  layout.panel = {0, 0, width + padding[3] + padding[1], y + padding[2]};
  auto reach = assets.outer;
  for (const auto &decoration : assets.decorations) {
    const auto box = native_canvas_decoration_box(decoration, layout.panel);
    reach[0] = std::max(reach[0], -box.y);
    reach[1] = std::max(reach[1], box.x + box.width - layout.panel.width);
    reach[2] = std::max(reach[2], box.y + box.height - layout.panel.height);
    reach[3] = std::max(reach[3], -box.x);
  }
  for (auto &edge : reach) edge = std::clamp(edge, 0.0, 500.0);
  if (assets.banner && !content.menu) {
    reach[0] = std::max(reach[0], assets.banner->height);
    if (assets.banner->candidate)
      layout.overlay = native_canvas_overlay_layout(cr, *assets.banner->candidate, content, layout,
          native_canvas_banner_box(*assets.banner, layout.panel));
    if (layout.overlay) {
      // 最小字号之后仍保留完整单行；字形外伸也计入窗口，交由既有工作区上限回退。
      for (const auto &box : {layout.overlay->box, layout.overlay->ink}) {
        reach[0] = std::max(reach[0], -box.y);
        reach[1] = std::max(reach[1], box.x + box.width - layout.panel.width);
        reach[2] = std::max(reach[2], box.y + box.height - layout.panel.height);
        reach[3] = std::max(reach[3], -box.x);
      }
    }
  }
  auto shift = [&](NativeCanvasRect &rect) { rect.x += reach[3]; rect.y += reach[0]; };
  shift(layout.panel); shift(layout.preedit); shift(layout.auxiliary);
  shift(layout.previous); shift(layout.next); shift(layout.page_divider);
  if (layout.overlay) { shift(layout.overlay->box); shift(layout.overlay->ink); }
  shift(layout.expand); shift(layout.viewport); shift(layout.scrollbar); shift(layout.thumb);
  for (auto &item : layout.grid) shift(item);
  for (auto &item : layout.items) {
    if (!content.horizontal) item.width = width;
    shift(item);
  }
  layout.width = std::ceil(layout.panel.width + reach[1] + reach[3]);
  layout.height = std::ceil(layout.panel.height + reach[0] + reach[2]);
  return layout;
}

inline int native_canvas_hit_skin(const NativeCanvasContent &content, const NativeCanvasLayout &layout,
                             double x, double y) {
  if (layout.expand.width > 0 && layout.expand.contains(x, y)) return kNativeCanvasHitExpand;
  if (layout.scrollbar.width > 0 && layout.scrollbar.contains(x, y)) return kNativeCanvasHitScrollbar;
  if (layout.viewport.height > 0 && !layout.viewport.contains(x, y)) return -1;
  for (std::size_t at = 0; at < layout.items.size(); ++at)
    if (layout.items[at].contains(x, y)) return static_cast<int>(at);
  for (std::size_t at = 0; at < layout.grid.size(); ++at)
    if (layout.grid[at].contains(x, y)) return static_cast<int>(content.items.size() + at);
  if (layout.pager.visible && content.previous && layout.previous.contains(x, y)) return kNativeCanvasHitPrevious;
  if (layout.pager.visible && content.next && layout.next.contains(x, y)) return kNativeCanvasHitNext;
  return -1;
}

// 逐角路径与来源一致：每角上限为短边的一半；0 仍是合法直角。
inline void native_canvas_round_rect(cairo_t *cr, const NativeCanvasRect &rect, std::array<double, 4> radii) {
  const double limit = std::max(0.0, std::min(rect.width, rect.height) / 2);
  for (auto &radius : radii) radius = std::clamp(radius, 0.0, limit);
  const auto [tl, tr, br, bl] = radii;
  const double right = rect.x + rect.width, bottom = rect.y + rect.height;
  constexpr double pi = 3.14159265358979323846;
  cairo_new_sub_path(cr);
  cairo_move_to(cr, rect.x + tl, rect.y);
  cairo_line_to(cr, right - tr, rect.y);
  if (tr > 0) cairo_arc(cr, right - tr, rect.y + tr, tr, -pi / 2, 0);
  cairo_line_to(cr, right, bottom - br);
  if (br > 0) cairo_arc(cr, right - br, bottom - br, br, 0, pi / 2);
  cairo_line_to(cr, rect.x + bl, bottom);
  if (bl > 0) cairo_arc(cr, rect.x + bl, bottom - bl, bl, pi / 2, pi);
  cairo_line_to(cr, rect.x, rect.y + tl);
  if (tl > 0) cairo_arc(cr, rect.x + tl, rect.y + tl, tl, pi, pi * 1.5);
  cairo_close_path(cr);
}

inline std::array<double, 4> native_canvas_item_corners(const NativeCanvasContent &content,
                                                       const NativeCanvasLayout &layout, std::size_t at) {
  const auto &metrics = layout.metrics;
  std::array<double, 4> radii;
  radii.fill(metrics.item_radius);
  if (!metrics.corners_follow_card || layout.items.empty()) return radii;
  const auto &item = layout.items[at];
  const auto &first = layout.items.front();
  double bottom = first.y;
  for (const auto &box : layout.items) bottom = std::max(bottom, box.y + box.height);
  const bool top = content.preedit.empty() && item.y <= first.y + 0.5;
  const bool last = layout.grid.empty() && item.y + item.height >= bottom - 0.5;
  const bool left = item.x <= first.x + 0.5;
  const bool end = item.x + item.width >= layout.panel.x + layout.panel.width - metrics.content[1] - 0.5;
  if (top && left) radii[0] = metrics.card_radius;
  if (top && end) radii[1] = metrics.card_radius;
  if (last && end) radii[2] = metrics.card_radius;
  if (last && left) radii[3] = metrics.card_radius;
  return radii;
}

inline void native_canvas_color(cairo_t *cr, const nlohmann::json &palette, const char *key,
                                const char *fallback, double alpha_scale = 1) {
  const auto color = theme_color(palette.value(key, nlohmann::json(fallback))).value_or(*theme_color(fallback));
  cairo_set_source_rgba(cr, ((color.rgb >> 16) & 255) / 255.0, ((color.rgb >> 8) & 255) / 255.0,
                       (color.rgb & 255) / 255.0, color.alpha / 255.0 * alpha_scale);
}

inline void native_canvas_image(cairo_t *cr, cairo_surface_t *image, const NativeCanvasRect &source,
                                const NativeCanvasRect &destination, double alpha = 1) {
  if (!image || source.width <= 0 || source.height <= 0 || destination.width <= 0 || destination.height <= 0) return;
  // 为每一块建立独立源表面，PAD 只延伸该块边缘，避免九宫格相邻块串色。
  auto *tile = cairo_surface_create_for_rectangle(image, source.x, source.y, source.width, source.height);
  cairo_save(cr);
  cairo_rectangle(cr, destination.x, destination.y, destination.width, destination.height);
  cairo_clip(cr);
  cairo_translate(cr, destination.x, destination.y);
  cairo_scale(cr, destination.width / source.width, destination.height / source.height);
  cairo_set_source_surface(cr, tile, 0, 0);
  cairo_pattern_set_extend(cairo_get_source(cr), CAIRO_EXTEND_PAD);
  cairo_paint_with_alpha(cr, std::clamp(alpha, 0.0, 1.0));
  cairo_restore(cr);
  cairo_surface_destroy(tile);
}

inline void paint_native_canvas_skin(cairo_t *cr, const NativeCanvasAssets &assets, const NativeCanvasContent &content,
                                 const NativeCanvasLayout &layout, const nlohmann::json &palette,
                                 std::uint64_t elapsed_ms = 0, NativeCanvasPointer pointer = {}) {
  const auto frame = [elapsed_ms](const NativeCanvasImage &image, const std::shared_ptr<const NativeCanvasAnimation> &animation) {
    return animation ? animation->frames[sample_native_canvas_animation(*animation, elapsed_ms).index].get() : image.get();
  };
  const auto widgets = [&](bool front) {
    if (content.menu) return;
    for (const auto &decoration : assets.decorations) {
      if ((decoration.z >= 1) != front) continue;
      auto box = native_canvas_decoration_box(decoration, layout.panel);
      auto *image = frame(decoration.image, decoration.animation);
      const double width = cairo_image_surface_get_width(image);
      const double height = cairo_image_surface_get_height(image);
      const double ratio = std::min(box.width / width, box.height / height);
      const NativeCanvasRect target{box.x + (box.width - width * ratio) / 2,
          box.y + (box.height - height * ratio) / 2, width * ratio, height * ratio};
      native_canvas_image(cr, image, {0, 0, width, height}, target);
    }
  };
  auto colors = palette;
  // alpha 0 在这些来源槽位表示跟随普通文字或 accent，不是隐藏文字。
  const auto inherit = [&](const char *key, const char *parent, bool transparent_follows = true) {
    const auto color = theme_color(colors.value(key, nlohmann::json{}));
    if (!color || (transparent_follows && color->alpha == 0)) colors[key] = colors.value(parent, nlohmann::json{});
  };
  inherit("preedit_text", "text", false); inherit("preedit_caret", "accent"); inherit("selected_bar", "accent");
  inherit("selected_text", "text"); inherit("selected_number", "number");
  if (content.menu) {
    for (const auto &[key, parent] : std::vector<std::pair<const char *, const char *>>{
        {"surface", "menu_surface"}, {"text", "menu_text"}, {"hover", "menu_hover"}, {"border", "menu_border"}})
      if (colors.contains(parent)) colors[key] = colors.at(parent);
    colors["preedit_text"] = colors.value("text", nlohmann::json{});
    colors["preedit_background"] = colors["preedit_divider"] = "#00000000";
  }
  widgets(false);
  const auto &panel = layout.panel;
  // 菜单不是卡片，不随候选窗不透明度变淡。
  const double card_alpha = content.menu ? 1 : std::clamp(assets.opacity, 0.0, 1.0);
  if (assets.panel && !content.menu) {
    auto *image = frame(assets.panel, assets.panel_animation);
    const auto axis = [](double length, double before, double after) {
      const double ratio = before + after > length ? length / (before + after) : 1;
      return std::array<double, 4>{0, before * ratio, length - after * ratio, length};
    };
    const auto sx = axis(cairo_image_surface_get_width(image), assets.slice[3], assets.slice[1]);
    const auto sy = axis(cairo_image_surface_get_height(image), assets.slice[0], assets.slice[2]);
    const double factor = std::min({1.0, panel.width / std::max(1.0, assets.width[3] + assets.width[1]),
                                   panel.height / std::max(1.0, assets.width[0] + assets.width[2])});
    const std::array<double, 4> dx{panel.x, panel.x + assets.width[3] * factor,
        panel.x + panel.width - assets.width[1] * factor, panel.x + panel.width};
    const std::array<double, 4> dy{panel.y, panel.y + assets.width[0] * factor,
        panel.y + panel.height - assets.width[2] * factor, panel.y + panel.height};
    for (int row = 0; row < 3; ++row) for (int col = 0; col < 3; ++col) {
      if (!assets.fill && row == 1 && col == 1) continue;
      native_canvas_image(cr, image, {sx[col], sy[row], sx[col + 1] - sx[col], sy[row + 1] - sy[row]},
          {dx[col], dy[row], dx[col + 1] - dx[col], dy[row + 1] - dy[row]}, card_alpha);
    }
  } else {
    native_canvas_color(cr, colors, "surface", "#f8f8f8", card_alpha);
    const double radius = layout.metrics.card_radius;
    native_canvas_round_rect(cr, panel, {radius, radius, radius, radius});
    cairo_fill(cr);
  }
  const auto &metrics = layout.metrics;
  const double border_width = content.menu ? 1 : assets.border_width;
  if ((!assets.panel || content.menu) && border_width > 0) {
    native_canvas_color(cr, colors, "border", "#00000000", card_alpha);
    const double half = border_width / 2, radius = std::max(0.0, metrics.card_radius - half);
    native_canvas_round_rect(cr, {panel.x + half, panel.y + half, std::max(0.0, panel.width - 2 * half),
        std::max(0.0, panel.height - 2 * half)}, {radius, radius, radius, radius});
    cairo_set_line_width(cr, border_width); cairo_stroke(cr);
  }
  const auto text = [&](const std::string &value, const NativeCanvasRect &rect, const std::string &font_name,
                        const char *key, const char *fallback, double max_width = -1) {
    auto *line = native_canvas_text_layout(cr, font_name, value, max_width);
    native_canvas_color(cr, colors, key, fallback);
    cairo_move_to(cr, rect.x, rect.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
  };
  if (!content.preedit.empty()) {
    auto background = layout.preedit;
    background.width = std::max(background.width, panel.x + panel.width - metrics.content[1] -
        metrics.preedit_margin[1] - background.x);
    native_canvas_color(cr, colors, "preedit_background", "#00000000");
    const double radius = metrics.item_radius;
    native_canvas_round_rect(cr, background, {radius, radius, radius, radius}); cairo_fill(cr);
    native_canvas_color(cr, colors, "preedit_divider", "#00000000");
    cairo_rectangle(cr, background.x, background.y + background.height - 1, background.width, 1); cairo_fill(cr);
    text(content.preedit, layout.preedit, metrics.font, "preedit_text", "#202020", layout.preedit_width);
    if (!content.menu && content.preedit_cursor >= 0 &&
        static_cast<std::size_t>(content.preedit_cursor) <= content.preedit.size() &&
        (static_cast<std::size_t>(content.preedit_cursor) == content.preedit.size() ||
         (static_cast<unsigned char>(content.preedit[content.preedit_cursor]) & 0xc0) != 0x80)) {
      auto *line = native_canvas_text_layout(cr, metrics.font, content.preedit, layout.preedit_width);
      PangoRectangle strong{}, weak{};
      pango_layout_get_cursor_pos(line, content.preedit_cursor, &strong, &weak);
      native_canvas_color(cr, colors, "preedit_caret", "#3584e4");
      cairo_rectangle(cr, layout.preedit.x + strong.x / static_cast<double>(PANGO_SCALE),
          layout.preedit.y + strong.y / static_cast<double>(PANGO_SCALE), 2,
          strong.height / static_cast<double>(PANGO_SCALE)); cairo_fill(cr);
      g_object_unref(line);
    }
  }
  const auto show_bar = colors.find("show_selected_bar");
  const bool selection_bar = show_bar != colors.end() && show_bar->is_boolean() && show_bar->get<bool>();
  const auto paint_item = [&](const NativeCanvasItem &item, NativeCanvasRect rect, double text_width,
                              int at, bool selected, std::array<double, 4> corners) {
    const bool pressed = static_cast<int>(at) == pointer.pressed;
    const bool hovered = static_cast<int>(at) == pointer.hovered;
    if (selected || pressed || hovered) {
      native_canvas_color(cr, colors, (selected || pressed) && !content.menu ? "selected" : "hover",
          selected || pressed ? "#dceaff" : "#00000010");
      native_canvas_round_rect(cr, rect, corners); cairo_fill(cr);
      if (!content.menu && at < static_cast<int>(content.items.size()) && (selected || pressed) && selection_bar) {
        native_canvas_color(cr, colors, "selected_bar", "#3584e4");
        const double width = metrics.bar_width, height = metrics.bar_height, radius = width / 2;
        native_canvas_round_rect(cr, {rect.x - (metrics.bar_inside ? 0 : width / 2),
            rect.y + std::max(0.0, (rect.height - height) / 2), width, height},
            {radius, radius, radius, radius}); cairo_fill(cr);
      }
    }
    const auto text_height = native_canvas_item_size(cr, metrics, item, content.horizontal, text_width).second;
    const auto label_size = native_canvas_text_size(cr, metrics.number_font, item.label);
    const double top = rect.y;
    const bool centered = metrics.custom_height || metrics.number_scale != 1 || !assets.font_family.empty();
    rect.x += metrics.number_inset;
    if (metrics.number_scale > 0 && !item.label.empty()) {
      rect.y = top + (centered ? (rect.height - label_size.second) / 2 : 4);
      text(item.label, rect, metrics.number_font, selected || pressed ? "selected_number" : "number", "#555555");
    }
    // 序号字号为 0 时不画字形，但来源仍保留最小 9 DIP 序号栏与间距。
    if (at >= static_cast<int>(content.items.size())) rect.x += layout.grid_label_width;
    else if (!item.label.empty()) rect.x += std::max(metrics.number_min_width,
        metrics.number_scale == 0 ? 0 : label_size.first) + metrics.number_gap;
    rect.y = top + (centered ? (rect.height - text_height) / 2 : 4);
    auto *line = native_canvas_item_layout(cr, metrics, item, content.horizontal, colors, selected || pressed, text_width);
    if (item.fixed && !content.menu && !selected && !pressed)
      cairo_set_source_rgb(cr, ((kCandidateFixedPositionColor >> 16) & 255) / 255.0,
          ((kCandidateFixedPositionColor >> 8) & 255) / 255.0, (kCandidateFixedPositionColor & 255) / 255.0);
    else
      native_canvas_color(cr, colors, selected || pressed ? "selected_text" : "text", "#202020");
    cairo_move_to(cr, rect.x, rect.y);
    pango_cairo_show_layout(cr, line);
    g_object_unref(line);
  };
  cairo_save(cr);
  if (layout.viewport.height > 0) {
    cairo_rectangle(cr, layout.viewport.x, layout.viewport.y, layout.viewport.width, layout.viewport.height);
    cairo_clip(cr);
  }
  for (std::size_t at = 0; at < content.items.size(); ++at)
    paint_item(content.items[at], layout.items[at], layout.item_text_widths[at], static_cast<int>(at),
        static_cast<int>(at) == content.selected, native_canvas_item_corners(content, layout, at));
  for (std::size_t at = 0; at < layout.grid.size(); ++at) {
    const auto &box = layout.grid[at];
    if (box.y + box.height <= layout.viewport.y || box.y >= layout.viewport.y + layout.viewport.height) continue;
    const double radius = metrics.item_radius;
    paint_item(content.grid_items[at], box, layout.grid_text_widths[at],
        static_cast<int>(content.items.size() + at), false, {radius, radius, radius, radius});
  }
  cairo_restore(cr);
  if (layout.expand.width > 0) {
    const auto &box = layout.expand;
    if (pointer.hovered == kNativeCanvasHitExpand || pointer.pressed == kNativeCanvasHitExpand) {
      native_canvas_color(cr, colors, pointer.pressed == kNativeCanvasHitExpand ? "selected" : "hover", "#00000010");
      const double radius = metrics.item_radius;
      native_canvas_round_rect(cr, box, {radius, radius, radius, radius}); cairo_fill(cr);
    }
    native_canvas_color(cr, colors, "number", "#555555");
    const double cx = box.x + box.width / 2, cy = box.y + box.height / 2;
    const double half = metrics.font_size * 0.2, direction = content.expanded ? -1 : 1;
    cairo_move_to(cr, cx - half, cy - direction * half / 2);
    cairo_line_to(cr, cx, cy + direction * half / 2);
    cairo_line_to(cr, cx + half, cy - direction * half / 2);
    cairo_set_line_width(cr, std::max(1.2, metrics.font_size / 13)); cairo_stroke(cr);
  }
  if (layout.thumb.width > 0) {
    native_canvas_color(cr, colors, "number", "#555555");
    cairo_push_group(cr);
    native_canvas_round_rect(cr, layout.thumb, {2, 2, 2, 2}); cairo_fill(cr);
    cairo_pop_group_to_source(cr);
    cairo_paint_with_alpha(cr, pointer.hovered == kNativeCanvasHitScrollbar || pointer.pressed == kNativeCanvasHitScrollbar ? 0.85 : 0.45);
  }
  text(content.auxiliary, layout.auxiliary, metrics.font, "secondary", "#555555", layout.auxiliary_width);
  if (layout.pager.visible) {
    const auto &pager = layout.pager;
    if (pager.divider_width > 0) {
      native_canvas_color(cr, colors, "page_arrow_divider", "#00000000");
      const auto &box = layout.page_divider;
      cairo_rectangle(cr, box.x, box.y, box.width, box.height); cairo_fill(cr);
    }
    for (const bool next : {false, true}) {
      const auto &box = next ? layout.next : layout.previous;
      const int part = next ? -3 : -2;
      const bool enabled = next ? content.next : content.previous;
      if (enabled && (pointer.pressed == part || pointer.hovered == part)) {
        const auto selected = theme_color(colors.value("selected", nlohmann::json{}));
        native_canvas_color(cr, colors, pointer.pressed == part && selected && selected->alpha > 0 ? "selected" : "hover",
            "#00000010");
        native_canvas_round_rect(cr, box, {pager.radius, pager.radius, pager.radius, pager.radius}); cairo_fill(cr);
      }
      const auto glyph = theme_color(colors.value(pager.triangle ? "text" : "number", nlohmann::json{}))
          .value_or(ThemeColor{pager.triangle ? 0x202020u : 0x555555u, 255});
      const double alpha = glyph.alpha / 255.0 * (enabled ? 1 : pager.triangle ? 0.25 : 0.3);
      cairo_set_source_rgba(cr, ((glyph.rgb >> 16) & 255) / 255.0,
          ((glyph.rgb >> 8) & 255) / 255.0, (glyph.rgb & 255) / 255.0, alpha);
      const double cx = box.x + box.width / 2, cy = box.y + box.height / 2;
      const double height = std::min(pager.glyph, box.height), width = pager.glyph_width();
      const double tip = cx + (next ? width / 2 : -width / 2), tail = 2 * cx - tip;
      if (pager.triangle) {
        // 三角逐角切圆后一次填充；禁用时不会因半透明描边重叠而变深。
        using Point = std::pair<double, double>;
        const std::array<Point, 3> corners{{{tip, cy}, {tail, cy - height / 2}, {tail, cy + height / 2}}};
        const double radius = std::clamp(pager.stroke, 0.0, height / 2);
        const auto toward = [radius](Point from, Point to) {
          const double dx = to.first - from.first, dy = to.second - from.second;
          const double length = std::hypot(dx, dy), t = length > 0 ? radius / length : 0;
          return Point{from.first + dx * t, from.second + dy * t};
        };
        const auto start = toward(corners[0], corners[1]);
        cairo_move_to(cr, start.first, start.second);
        for (int at = 1; at <= 3; ++at) {
          const auto corner = corners[at % 3];
          const auto before = toward(corner, corners[at - 1]);
          const auto after = toward(corner, corners[(at + 1) % 3]);
          cairo_line_to(cr, before.first, before.second);
          cairo_curve_to(cr, before.first + (corner.first - before.first) * 2 / 3,
              before.second + (corner.second - before.second) * 2 / 3,
              after.first + (corner.first - after.first) * 2 / 3,
              after.second + (corner.second - after.second) * 2 / 3, after.first, after.second);
        }
        cairo_close_path(cr); cairo_fill(cr);
      } else {
        cairo_move_to(cr, tail, cy - height / 2); cairo_line_to(cr, tip, cy);
        cairo_line_to(cr, tail, cy + height / 2);
        cairo_set_line_width(cr, pager.stroke); cairo_set_line_join(cr, CAIRO_LINE_JOIN_MITER);
        cairo_stroke(cr);
      }
    }
  }
  widgets(true);
  if (assets.banner && !content.menu) {
    const auto &banner = *assets.banner;
    auto *image = frame(banner.image, banner.animation);
    const double width = cairo_image_surface_get_width(image), height = cairo_image_surface_get_height(image);
    const double scale = std::min({banner.width / width, banner.height / height, panel.width / width});
    const double align = banner.align == "left" ? 0 : banner.align == "center" ? 0.5 : 1;
    native_canvas_image(cr, image, {0, 0, width, height},
        {panel.x + (panel.width - width * scale) * align, panel.y - height * scale, width * scale, height * scale});
    if (layout.overlay && banner.candidate) {
      const auto &overlay = *layout.overlay;
      auto *line = native_canvas_overlay_line(cr, layout.metrics.font, overlay.text, overlay.font_size);
      native_canvas_color(cr, {{"text", banner.candidate->color}}, "text", "#FFFFFFFF");
      cairo_move_to(cr, overlay.box.x, overlay.box.y);
      pango_cairo_show_layout(cr, line);
      g_object_unref(line);
    }
  }
}

// 候选窗摆在哪儿：规则是与 macOS 共用的 CandidateOrigin（光标下方 4pt，放不下按这次显示以来最高的一页翻到光标上方，装饰透明带和左、下外留白不占光标间隙，最后夹进工作区）。X11 的坐标 y 朝下、单位是物理像素，这里按显示倍率换成逻辑单位、翻转 y，算完再换回来。`card_top`、`card_left`、`card_bottom` 是窗口里卡片外的留白（逻辑单位），`flip_height` 是记住的最高一页（逻辑单位，0 表示不记）。返回窗口左上角。
inline std::pair<int, int> native_canvas_window_origin(double scale, int caret_x, int caret_y, int caret_height,
    int width, int height, int work_x, int work_y, int work_width, int work_height,
    double flip_height = 0, double card_top = 0, double card_left = 0, double card_bottom = 0) {
  namespace shared = msime::candidate;
  if (!(scale > 0) || !std::isfinite(scale)) scale = 1;
  const shared::CandidateRect caret{caret_x / scale, caret_y / scale, 1 / scale, std::max(1, caret_height) / scale};
  const shared::CandidateRect work{work_x / scale, work_y / scale, work_width / scale, work_height / scale};
  const double window_width = width / scale, window_height = height / scale;
  const auto origin = shared::CandidateOrigin(shared::CandidateFlipY(caret), window_width, window_height,
      shared::CandidateFlipY(work), flip_height, card_top, card_left, card_bottom);
  int x = static_cast<int>(std::lround(origin.x * scale));
  int y = static_cast<int>(std::lround(-(origin.y + window_height) * scale));
  x = std::clamp(x, work_x, std::max(work_x, work_x + work_width - width));
  y = std::clamp(y, work_y, std::max(work_y, work_y + work_height - height));
  return {x, y};
}

// ---- 默认渲染器：水杉主题和非 canvas 皮肤包 ----

// X11 上原生画布是 Fcitx5 候选窗的默认画法；`MSIME_FCITX5_CANDIDATE_RENDERER=classicui` 是逃生口，整套退回 classicui 主题（canvas 皮肤包也画成 classicui 的静态面板）。其他值和没设时都用原生画布。
inline bool native_canvas_renderer_enabled(const char *setting) {
  return !setting || std::string_view(setting) != "classicui";
}

// 主题配色交给原生画布的调色板：先铺 Linux 的 Adwaita token（system 主题不带调色板，自定义主题没设的槽位也一样），再盖上共享层解析出的候选配色和 candidate_details。与 classicui 不同，这里保留半透明槽位的 alpha，由画布自己合成。
inline nlohmann::json native_canvas_theme_palette(const nlohmann::json &document, bool dark) {
  const auto native = candidate_native_palette(dark);
  const auto hex = [](std::uint32_t rgb, std::uint8_t alpha = 0xff) {
    char text[10];
    if (alpha == 0xff) std::snprintf(text, sizeof text, "#%06X", rgb & 0xffffffu);
    else std::snprintf(text, sizeof text, "#%06X%02X", rgb & 0xffffffu, alpha);
    return std::string(text);
  };
  // 悬停：照 macOS 的 token，浅色时黑 6%、深色时白 10%。
  nlohmann::json palette = {
      {"surface", hex(native.surface)}, {"text", hex(native.text)}, {"number", hex(native.number)},
      {"secondary", hex(native.number)}, {"accent", hex(native.accent)}, {"selected", hex(native.selected)},
      {"selected_text", hex(native.selected_text)}, {"selected_number", hex(native.selected_number)},
      {"border", hex(native.border)}, {"hover", dark ? hex(0xffffff, 0x1a) : hex(0x000000, 0x0f)},
      {"selected_bar", hex(kLinuxAccent)}};
  if (document.is_object()) {
    const auto themed = native_canvas_declared_palette(document);
    for (const auto &[key, value] : themed.items())
      if (theme_color(value) || value.is_boolean()) palette[key] = value;
    // 选中竖条与 macOS 一样没设时跟随主题的 accent（selectedBar ?? accent）；只有主题自己没给 accent 时才用 Adwaita 的蓝。
    if (!theme_color(themed.value("selected_bar", nlohmann::json{})) && theme_color(themed.value("accent", nlohmann::json{})))
      palette["selected_bar"] = themed.at("accent");
  }
  return palette;
}

// 首行 logo：hicolor 主题里本包安装的应用图标，取不小于 64 像素的一档（没有就取最大的一档），画的时候再缩到 16pt 乘缩放与显示倍率，HiDPI 下也清楚。读不到时不画 logo。
inline NativeCanvasImage load_native_canvas_logo(const std::filesystem::path &icons) {
  std::filesystem::path chosen;
  for (const int size : {16, 32, 48, 64, 128, 256}) {
    const auto name = std::to_string(size) + "x" + std::to_string(size);
    const auto file = icons / name / "apps" / (MSIME_EDITION_ICON ".png");
    std::error_code error;
    if (!std::filesystem::is_regular_file(file, error)) continue;
    chosen = file;
    if (size >= 64) break;
  }
  if (chosen.empty()) return {};
  NativeCanvasImage image(cairo_image_surface_create_from_png(chosen.c_str()), cairo_surface_destroy);
  if (cairo_surface_status(image.get()) != CAIRO_STATUS_SUCCESS || cairo_image_surface_get_width(image.get()) <= 0) return {};
  return image;
}

// 主题画的卡片几何，单位是逻辑像素，已乘 candidate_scale_percent（描边不缩放，与 macOS 相同）。
struct NativeCanvasThemeStyle {
  double scale = 1, opacity = 1;
  double padding = 4, card_radius = 10, item_radius = 6, border_width = 0;
  bool user_radius = false, show_selected_bar = false;
  double min_width = 0;
  std::optional<CandidateSkinDecoration> decoration;
  std::optional<CandidateSkinBackground> background;
};

// 水杉自己的主题（system、内置主题、自定义主题的取色器）和非 canvas 皮肤包：没有九宫格贴图，按主题配色画 macOS 结构的卡片。`package` 是 msime_client_skin_package 给出的那个包（没画包时为空对象）：取它的共享版式（resolvedLayout）、字体、`[animation]` 和装饰图待机动效；装饰图本身按宿主目录给出的绝对路径读，并解出 APNG、GIF、WebP 的各帧。装饰图读不出时只少装饰，卡片照画。
inline std::shared_ptr<NativeCanvasAssets> native_canvas_theme_assets(const NativeCanvasThemeStyle &style,
                                                                     const nlohmann::json &package) {
  auto result = std::make_shared<NativeCanvasAssets>();
  result->chrome = true;
  result->scale = style.scale;
  result->opacity = style.opacity;
  result->content = {style.padding, style.padding, style.padding, style.padding};
  result->card_radius = style.card_radius;
  result->item_radius = style.item_radius;
  result->border_width = style.border_width;
  result->user_radius = style.user_radius;
  result->show_selected_bar = style.show_selected_bar;
  result->min_width = style.min_width;
  if (package.is_object()) {
    if (const auto layout = package.find("resolvedLayout"); layout != package.end() && layout->is_object()) {
      result->geometry = layout->value("geometry", nlohmann::json::object());
      result->horizontal = layout->value("horizontal", nlohmann::json::object());
      result->vertical = layout->value("vertical", nlohmann::json::object());
      result->fit_content = layout->value("fitContent", false);
      result->expand_rows = native_canvas_expand_rows(*layout);
    }
    result->font_family = package.value("fontFamily", std::string{});
    read_native_canvas_animation(*result, package);
  }
  if (style.decoration) {
    const auto bytes = read_fcitx_canvas_image(style.decoration->image);
    std::uint64_t budget = 8000000;
    if (auto image = bytes ? decode_native_canvas_animation(*bytes, budget) : nullptr) {
      result->band.image = std::move(image);
      result->band.top = style.decoration->top_dip * style.scale;
      result->band.width = style.decoration->width_dip * style.scale;
      result->band.align = style.decoration->align == CandidateSkinAlign::left ? msime::candidate::DecorationAlign::left
                           : style.decoration->align == CandidateSkinAlign::center ? msime::candidate::DecorationAlign::center
                                                                                   : msime::candidate::DecorationAlign::right;
      if (package.is_object()) {
        result->band.motion = msime::candidate::DecorationMotionNamed(package.value("decorationMotion", std::string{}));
        result->band.period_ms = package.value("decorationMotionPeriodMs", 0.0);
        result->band.candidate = native_canvas_overlay(package.value("decorationCandidate", nlohmann::json{}));
        if (result->band.candidate) {
          auto &overlay = *result->band.candidate;
          overlay.fontSizeDip *= style.scale;
          overlay.minFontSizeDip *= style.scale;
          overlay.maxWidthDip *= style.scale;
          overlay.offsetXDip *= style.scale;
          overlay.offsetYDip *= style.scale;
        }
      }
      // 装饰图比卡片宽时卡片跟着变宽（macOS 把 decorationWidthDip 算进最小宽度）。
      result->min_width = std::max(result->min_width, result->band.width);
    }
  }
  // 背景图读不出或解不开时只少背景，卡片照画；不透明度为 0 时不读。
  if (style.background && style.background->opacity > 0) {
    const auto bytes = read_fcitx_canvas_image(style.background->image);
    NativeCanvasImage image(bytes ? decode_fcitx_canvas_image(*bytes) : nullptr, cairo_surface_destroy);
    if (image && cairo_surface_status(image.get()) == CAIRO_STATUS_SUCCESS && cairo_image_surface_get_width(image.get()) > 0 &&
        cairo_image_surface_get_height(image.get()) > 0)
      result->background = {std::move(image), style.background->fit, style.background->opacity};
  }
  return result;
}

} // msime::linux_host 命名空间

// macOS 结构的画法和按资产分派的入口（measure_native_canvas、paint_native_canvas、native_canvas_hit）。
#include "NativeCanvasChrome.h"
