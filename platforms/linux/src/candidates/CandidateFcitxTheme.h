#pragma once

#include <array>
#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cerrno>
#include <cstdio>
#include <filesystem>
#include <fcntl.h>
#include <fstream>
#include <functional>
#include <iterator>
#include <optional>
#include <sstream>
#include <string>
#include <string_view>
#include <sys/stat.h>
#include <system_error>
#include <unistd.h>
#include <utility>
#include <vector>

#include "CandidateColors.h"
#include "CandidatePalette.h"
#include "FcitxThemeImages.h"
#include "AtomicWrite.h"
#include "../core/LinuxEdition.h"

namespace msime::linux_host {

// Fcitx5's classic UI draws the candidate list from a named theme. MSIME publishes its palette as a theme of its own, so the list looks the same as on IBus and Windows, while a theme the user picked in fcitx5-configtool is never replaced: only Fcitx5's stock themes, or MSIME's own, are taken over.
// 主题名与 Fcitx5 插件名相同（LinuxEdition.h）：两个版本的插件在同一个 fcitx5 里各写各的主题，不互相覆盖。
inline constexpr std::string_view kFcitxCandidateTheme = MSIME_EDITION_FCITX5_ADDON;

inline bool fcitx_theme_replaceable(std::string_view current) {
  return current.empty() || current == "default" || current == "default-dark" ||
         current == kFcitxCandidateTheme;
}

// Whether the classic UI draws MSIME's theme in both appearances. The light `Theme` and, on Fcitx5 releases that have one, the `DarkTheme` are taken over separately and only while each holds a stock theme, so a user's own dark theme stays in place and is what Fcitx5 draws in dark mode.
inline bool fcitx_candidate_theme_drawn(std::string_view theme, const std::string *dark_theme) {
  return fcitx_theme_replaceable(theme) && (!dark_theme || fcitx_theme_replaceable(*dark_theme));
}

inline std::string fcitx_theme_color(std::uint32_t rgb, bool transparent = false) {
  char buffer[10];
  std::snprintf(buffer, sizeof buffer, transparent ? "#%06x00" : "#%06x", rgb & 0xffffffu);
  return buffer;
}

// An installed skin's decoration as the theme draws it (see stage_fcitx_overlay): the name of its copy in the theme directory, the transparent band reserved for it above the card, the height the copy is drawn at where this host can read it, and its alignment.
struct FcitxThemeOverlay {
  std::string file;
  int band = 0;
  std::optional<int> height;
  CandidateSkinAlign align = CandidateSkinAlign::right;
};

// 皮肤背景图在主题目录里的副本（background-<内容哈希>.png 与 @2x），由 write_fcitx_candidate_theme 生成。
struct FcitxThemeBackground {
  std::string file;
};

inline constexpr std::string_view kFcitxBackgroundPrefix = "background-";

// 把背景图按 fit 和不透明度画成 width×height（逻辑像素）的 1x 与 2x 两张 PNG；读不出或解不开时返回 `nullopt`。需要 cairo 和共享层的图片解码器，由链接了它们的 addon 传入（FcitxCanvasPanel.h 的 render_fcitx_background）。
using FcitxBackgroundRenderer = std::function<std::optional<std::pair<std::string, std::string>>(
    const CandidateSkinBackground &background, int width, int height, double opacity)>;

// The @2x twin a resampled decoration is written with: decoration-<hash>.png beside decoration-<hash>@2x.png, the naming the generated shapes use.
inline std::string fcitx_overlay_2x_name(const std::string &file) {
  constexpr std::string_view png = ".png";
  if (file.size() <= png.size() || file.compare(file.size() - png.size(), png.size(), png) != 0) return {};
  return file.substr(0, file.size() - png.size()) + "@2x.png";
}

// PNG 保持比例缩放到 `width` 宽、至多 `room` 高，失败返回 `nullopt`；classicui 按图片像素绘制，缩放器由链接 cairo 的 addon 传入，见 `FcitxThemeOverlayScale.h`。
using FcitxOverlayScaler = std::function<std::optional<std::string>(const std::string &png, int width, int room)>;

// An image the theme draws with, generated from the colours: its file name in the theme directory and its PNG bytes.
struct FcitxThemeImage {
  std::string file;
  std::string bytes;
};

// theme.conf and the images it names.
struct FcitxThemeFiles {
  std::string conf;
  std::vector<FcitxThemeImage> images;
};

// 已解码并规范化的 canvas 面板；四边顺序为上、右、下、左，classicui 继续负责动态窗口大小。
struct FcitxCanvasPanel {
  std::string file;
  std::array<int, 4> margin{};
  std::array<int, 4> content{};
  std::array<int, 4> outer{};
  std::vector<FcitxThemeImage> images;
};

inline constexpr std::string_view kFcitxCanvasPrefix = "canvas-";

template <typename ShapeNames>
inline void fcitx_collect_shape_names(const FcitxThemeFiles &theme, ShapeNames &names) {
  names.reserve(theme.images.size());
  for (const auto &image : theme.images) names.push_back(image.file);
}

// Generated images are named shape-<content hash>.png with an @2x copy beside each: changed colours give new names, so theme.conf changes with them and the classic UI loads the new pictures rather than ones it already holds under the old names.
inline constexpr std::string_view kFcitxShapePrefix = "shape-";

// 候选卡片的几何照 macOS 候选窗（design-tokens-desktop §1.2 的 macOS 一栏，CandidateSkin.cpp 的 SkinTokens、CandidateChrome.h）：10 px 卡片圆角、4 px 内边距（从卡片外沿量起，1 px 发丝线在其中）、6 px 行圆角，行内文字左右各留 8 px、上下各留 6 px，序号与 logo 也取 macOS 的尺寸。配色仍是 Linux 的 Adwaita token（CandidatePalette.h）。macOS 的投影由系统画，classicui 只能画进图片里：这里画成 4 px 向下、sigma 5、22% 黑，阴影边距也就是 Wayland 合成器放面板时的偏移，所以画得比设计稿的 0 6px 18px 紧。这些都是 100% 大小时的长度，`fcitx_panel_metrics` 按 candidate_scale_percent 缩放阴影以外的长度。
struct FcitxPanelGeometry {
  static constexpr int radius = 10;
  static constexpr int padding = 4;
  static constexpr int shadow_left = 12;
  static constexpr int shadow_right = 12;
  static constexpr int shadow_top = 8;
  static constexpr int shadow_bottom = 16;
  static constexpr int shadow_offset = 4;
  static constexpr double shadow_sigma = 5.0;
  static constexpr float shadow_alpha = 0.22f;
  static constexpr int item_radius = 6;
  static constexpr int item_left = 8;
  static constexpr int item_right = 8;
  static constexpr int item_vertical = 6;
  static constexpr int page_button = 16;
  // The brand mark leading the header row, at the size and gap macOS draws it (CandidateTypography.h).
  static constexpr int logo_side = 16;
  static constexpr int logo_gap = 6;
  // classicui 主题的 LabelTextSizeFactor 是百分比：序号按候选字号的 80% 画，与 macOS 的 MSIMECandidateNumberScale 相同。
  static constexpr int label_size_percent = 80;
};

// 用户在设置里调的候选窗样式（共享偏好），classicui 主题照 macOS 的 StyledCandidateSkin 应用它们：`scale` 是 candidate_scale_percent，乘到阴影以外的每一个长度上（圆角、内边距、行边距、翻页按钮、logo、装饰图），字号已经在字体描述里按它放大；`padding` 是 candidate_padding（0–16 px），代替主题的 4 px 内边距；`opacity` 是 candidate_opacity_percent，只淡化卡片底色和描边，文字、序号和选中高亮保持不透明；`page_arrows` 是 show_candidate_page_number，关掉时不画 ‹ ›（翻页键和滚轮照常），和 macOS 一起隐藏「1 / 3」与箭头一样；`item_radius` 是皮肤包的 item_corner_radius_dip。缺省值画出的主题与没有这些设置时完全相同。
struct FcitxCandidateStyle {
  double scale = 1.0;
  std::optional<double> padding;
  double opacity = 1.0;
  bool page_arrows = true;
  std::optional<double> item_radius;
  // candidate_layout 是否横排：只用来估计皮肤背景图按多大的卡片排（fcitx_background_box）。
  bool horizontal = false;
};

// 从偏好文档读样式，范围与共享层的校验相同（缩放 50–200%、不透明度 50–100%、内边距 0–16），超出范围或类型不对的值按没设处理。皮肤包的行圆角不在偏好里，由调用方另外填。
inline FcitxCandidateStyle read_fcitx_candidate_style(const nlohmann::json &preferences) {
  FcitxCandidateStyle style;
  if (!preferences.is_object()) return style;
  const auto integer = [&](const char *key) -> std::optional<long long> {
    const auto value = preferences.find(key);
    if (value == preferences.end() || !value->is_number_integer()) return std::nullopt;
    return value->get<long long>();
  };
  if (const auto scale = integer("candidate_scale_percent"); scale && *scale >= 50 && *scale <= 200)
    style.scale = static_cast<double>(*scale) / 100.0;
  if (const auto opacity = integer("candidate_opacity_percent"); opacity && *opacity >= 50 && *opacity <= 100)
    style.opacity = static_cast<double>(*opacity) / 100.0;
  if (const auto padding = integer("candidate_padding"); padding && *padding >= 0 && *padding <= 16)
    style.padding = static_cast<double>(*padding);
  if (const auto pages = preferences.find("show_candidate_page_number"); pages != preferences.end() && pages->is_boolean())
    style.page_arrows = pages->get<bool>();
  if (const auto layout = preferences.find("candidate_layout"); layout != preferences.end() && *layout == "horizontal")
    style.horizontal = true;
  return style;
}

// 按样式缩放后的几何，单位是 classicui 的逻辑像素。阴影（边距、偏移、模糊和透明度）不缩放，见 fcitx_panel_metrics。
struct FcitxPanelMetrics {
  double scale = 1.0;
  int padding = FcitxPanelGeometry::padding;
  int shadow_left = FcitxPanelGeometry::shadow_left;
  int shadow_right = FcitxPanelGeometry::shadow_right;
  int shadow_top = FcitxPanelGeometry::shadow_top;
  int shadow_bottom = FcitxPanelGeometry::shadow_bottom;
  int shadow_offset = FcitxPanelGeometry::shadow_offset;
  double shadow_sigma = FcitxPanelGeometry::shadow_sigma;
  int item_left = FcitxPanelGeometry::item_left;
  int item_right = FcitxPanelGeometry::item_right;
  int item_vertical = FcitxPanelGeometry::item_vertical;
  int page_button = FcitxPanelGeometry::page_button;
  int logo_side = FcitxPanelGeometry::logo_side;
  int logo_gap = FcitxPanelGeometry::logo_gap;
};

inline FcitxPanelMetrics fcitx_panel_metrics(const FcitxCandidateStyle &style = {}) {
  using G = FcitxPanelGeometry;
  FcitxPanelMetrics metrics;
  metrics.scale = std::isfinite(style.scale) ? std::clamp(style.scale, 0.5, 2.0) : 1.0;
  const auto length = [&](double value) { return std::max(0, static_cast<int>(std::lround(value * metrics.scale))); };
  metrics.padding = length(style.padding ? std::clamp(*style.padding, 0.0, 16.0) : G::padding);
  // 阴影边距和模糊保持 100% 时的值：Wayland 上 classicui 不读 ShadowMargin，按窗口左上角放面板，阴影边越宽候选窗离光标越远，缩放不能让这段偏移跟着变大。macOS 的投影由系统画，也不随候选窗缩放。
  metrics.item_left = length(G::item_left);
  metrics.item_right = length(G::item_right);
  metrics.item_vertical = std::max(1, length(G::item_vertical));
  metrics.page_button = std::max(8, length(G::page_button));
  metrics.logo_side = std::max(8, length(G::logo_side));
  metrics.logo_gap = length(G::logo_gap);
  return metrics;
}

// 皮肤背景图按多大的卡片排。classicui 的背景图只能是一张固定大小、居中贴在卡片上并裁到卡片里的叠图，画主题时还不知道卡片会多大，所以按这个布局下常见的一页候选估一个尺寸：竖排约 240×280，横排约 560×80（100% 时的逻辑像素，随缩放变化）。卡片比它大时四周露出卡片底色，比它小时只看得到中间一块。
inline std::pair<int, int> fcitx_background_box(const FcitxCandidateStyle &style) {
  const double scale = fcitx_panel_metrics(style).scale;
  const auto length = [scale](double value) { return std::max(1, static_cast<int>(std::lround(value * scale))); };
  return style.horizontal ? std::pair{length(560), length(80)} : std::pair{length(240), length(280)};
}

// 卡片圆角、内容到卡片外沿的距离和选中行圆角，主题图片、ContentMargin 和装饰图的定位共用这一份。
struct FcitxCardInset {
  int radius = 0;
  int pad = 0;
  int item_radius = 0;
};

// 内边距照 macOS 的 CandidateInset 取描边宽度与内边距中较大的一个（描边画在内边距里面），选中高亮因此不会压住描边。
//
// 行圆角照 macOS 的 StyledCandidateSkin 推导：`user_radius` 表示 `corner_radius` 是用户自己设的卡片圆角（不是皮肤包的），这时行圆角是卡片圆角减去内边距，和卡片同心；只调了内边距时行圆角至少大到高亮的角不伸出卡片；都没调时用皮肤包的 item_corner_radius_dip（`style.item_radius`），没有就是 6 px。classicui 的高亮九宫格固定切片就是行的文字边距，所以行圆角最多等于上下文字边距（6 px），再圆就画不出来。
//
// 被这个上限截短时，macOS 靠同心保证的「高亮不伸出卡片圆角」不再成立：卡片圆角大（例如用户设 32）、内边距只有 4 px 时，首行或末行高亮的角会伸到卡片外面的阴影上。所以这时把内边距加大到高亮圆弧在对角线上不越过卡片圆弧（描边按半宽算，与上面的下限同一种量法）：pad ≥ b/2 + (R − b/2 − r)(1 − 1/√2)。默认的 10 px 卡片和 6 px 行圆角用不到这一步。
inline FcitxCardInset fcitx_card_inset(const CandidateColors &colors, const FcitxCandidateStyle &style = {},
                                       const std::optional<double> &corner_radius = std::nullopt,
                                       bool user_radius = false) {
  using G = FcitxPanelGeometry;
  const auto metrics = fcitx_panel_metrics(style);
  const int border_width = colors.border ? std::max(0, colors.border_width) : 0;
  FcitxCardInset inset;
  inset.pad = std::max(border_width, metrics.padding);
  // The user's or the skin's radius (0-32, resolved by the caller) replaces the design's; the corner slices grow and shrink with it.
  inset.radius = static_cast<int>(
      std::lround((corner_radius ? std::clamp(*corner_radius, 0.0, 32.0) : static_cast<double>(G::radius)) * metrics.scale));
  const int item_limit = std::min({metrics.item_vertical, metrics.item_left, metrics.item_right});
  double row_radius = (style.item_radius ? std::clamp(*style.item_radius, 0.0, 16.0) : static_cast<double>(G::item_radius)) * metrics.scale;
  const double half_border = border_width / 2.0;
  if (user_radius) {
    row_radius = std::max(0, inset.radius - inset.pad);
  } else if (style.padding) {
    row_radius = std::max(row_radius, inset.radius - half_border - (inset.pad - half_border) * (2.0 + std::sqrt(2.0)));
  }
  inset.item_radius = std::clamp(static_cast<int>(std::lround(row_radius)), 0, item_limit);
  const double arc = 1.0 - 1.0 / std::sqrt(2.0);
  const double needed = half_border + std::max(0.0, inset.radius - half_border - inset.item_radius) * arc;
  inset.pad = std::max(inset.pad, static_cast<int>(std::ceil(needed - 1e-9)));
  return inset;
}

// 卡片外沿到内容的距离（见 fcitx_card_inset）。装饰底边定位和可用高度共用它，避免缩放后越界或偏移。
inline int fcitx_card_padding(const CandidateColors &colors, const FcitxCandidateStyle &style = {},
                              const std::optional<double> &corner_radius = std::nullopt, bool user_radius = false) {
  return fcitx_card_inset(colors, style, corner_radius, user_radius).pad;
}

// The brand mark as the theme draws it: the installed application icon at 1x (logo_side square) and 2x (twice that), read by load_fcitx_theme_logo.
struct FcitxThemeLogo {
  FcitxPixels one;
  FcitxPixels two;
};

// The design's Linux menu: #FFFFFF or #383838 with a 12 px radius, 6 px padding and a 1 px rgba(0,0,0,.08) ring, 6 px rounded items whose hover is the platform hover (6% black, 8% white) under unchanged text, and hairline separators with 6 px above and below. The classic UI has no shadow margin for menus, so their shadow is not drawn.
struct FcitxMenuGeometry {
  static constexpr int radius = 12;
  static constexpr int padding = 6;
  static constexpr int item_radius = 6;
  static constexpr int item_horizontal = 6;
  static constexpr int item_vertical = 8;
  static constexpr int separator_gap = 6;
  static constexpr int check_width = 22;
  static constexpr int glyph_size = 16;
};

struct FcitxMenuPalette {
  std::uint32_t background;
  std::uint32_t text;
  std::uint32_t hover;
  std::uint32_t separator;
  // The outline as a flat colour, composited over the background, for the classic UI's BorderColor.
  std::uint32_t ring;
  // The same outline as the menu image strokes it, alpha kept, so the rounded corners blend it over the fill the way the design's ring does; an alpha of 0 draws no outline.
  ThemeColor ring_stroke;
};

// The menu palette for the resolved appearance, derived from the theme's candidate palette as THEME_CONTRACT §3 maps the floating menus: surface to the menu background, text to the items, hover to the hovered item, border to the separators and the outline, each composited over the menu background as the classic UI draws opaque colours. A slot the theme leaves null, and every slot under `system`, takes the design's Linux menu token instead, and the translucent tokens (hover, separator, ring) are composited over whatever background is drawn. A fully transparent border means the theme draws no outline, so the menu has none; its separators fall back to the native hairline, since a menu needs them to be read. The contract's checked item (selected with selected_text) has no counterpart: the classic UI marks a checked item only with the CheckBox image, which is drawn in the item text colour.
inline FcitxMenuPalette fcitx_menu_palette(bool dark, const CandidateMenuSlots &slots = {}) {
  const std::uint32_t native_background = dark ? 0x383838u : 0xFFFFFFu;
  const auto background =
      slots.surface ? composite_color(slots.surface->rgb, slots.surface->alpha, native_background) : native_background;
  const auto over = [&](const std::optional<ThemeColor> &slot, std::uint32_t rgb, std::uint8_t alpha) {
    return slot ? composite_color(slot->rgb, slot->alpha, background) : composite_color(rgb, alpha, background);
  };
  const bool outlined = !slots.border || slots.border->alpha > 0;
  const ThemeColor ring_stroke = slots.border ? *slots.border : ThemeColor{0x000000u, 0x14};
  return FcitxMenuPalette{
      background,
      dark ? over(slots.text, 0xFFFFFFu, 0xFF) : over(slots.text, 0x000000u, 0xD1),
      dark ? over(slots.hover, 0xFFFFFFu, 0x14) : over(slots.hover, 0x000000u, 0x0F),
      // 皮肤包的工具栏分隔线色（slots.divider）优先；没有时分隔线跟随外框。
      slots.divider ? over(slots.divider, 0, 0) : over(outlined ? slots.border : std::nullopt, 0x000000u, dark ? 0x5C : 0x1A),
      outlined ? composite_color(ring_stroke.rgb, ring_stroke.alpha, background) : background,
      ring_stroke,
  };
}

inline std::string fcitx_content_hash(std::string_view bytes) {
  std::uint64_t hash = 14695981039346656037ull;
  for (const char value : bytes) {
    hash ^= static_cast<unsigned char>(value);
    hash *= 1099511628211ull;
  }
  char digest[17];
  std::snprintf(digest, sizeof digest, "%016llx", static_cast<unsigned long long>(hash));
  return digest;
}

// Draw one image at 1x and 2x, add both to `images` and return the 1x name theme.conf refers to.
template <typename Draw>
std::string fcitx_add_shape(std::vector<FcitxThemeImage> &images, int width, int height, Draw draw) {
  FcitxCanvas single(width, height, 1);
  draw(single);
  FcitxCanvas doubled(width, height, 2);
  draw(doubled);
  auto one = fcitx_png_encode(single);
  auto two = fcitx_png_encode(doubled);
  const auto stem = std::string(kFcitxShapePrefix) + fcitx_content_hash(one + two);
  images.push_back({stem + ".png", std::move(one)});
  images.push_back({stem + "@2x.png", std::move(two)});
  return stem + ".png";
}

inline std::string fcitx_margin(int left, int right, int top, int bottom) {
  return "Left=" + std::to_string(left) + "\nRight=" + std::to_string(right) + "\nTop=" + std::to_string(top) +
         "\nBottom=" + std::to_string(bottom) + "\n\n";
}

// The classic UI theme format has no accent colour and no hover state separate from the highlight, so the accent is not drawn; the candidate number takes its own colour only on Fcitx5 releases with CandidateLabelColor, and follows the text on older ones, which ignore the key. Everything else the format can carry comes from the same resolved theme IBus draws. Without a selected fill the highlight is transparent and the selected row is told apart by its text colour alone. The menu (the tray and status-area menus the classic UI draws) derives from the same theme's candidate palette as THEME_CONTRACT §3 maps it, through fcitx_menu_palette, and keeps the Linux menu tokens for every slot the theme leaves null.
//
// The card, its rounded highlight and the menu are nine-slice images drawn by fcitx_add_shape; each section also keeps its flat colour, which the classic UI draws instead when it cannot load the image. Where no compositor runs on X11 the classic UI has no alpha channel, so the transparent corners and shadow show black there, as they do for any Fcitx5 theme with rounded images.
//
// The brand mark leads the header row as on every other host, but the classic UI lays every row out with the same content margin and has no slot for an image beside the preedit. So the mark is painted into the card image's top-left corner slice, which is never stretched, at the content's top-left corner and the text margin's top, and the content margin grows by the mark and its gap: the preedit follows the mark as the design's header does, and the candidate rows below keep the same left edge as the preedit. Without the mark the theme is unchanged.
//
// 装饰作为背景 Overlay：透明顶带增加面板与九宫格顶部固定切片高度，阴影和内容边距同步增加，让 X11 卡片仍贴光标；装饰底边伸入卡片一个内边距，画在候选前但止于内容起点。PNG 按 `width_dip` 缩放并生成 @2x，过高则继续等比缩小；无法缩放但已知高度的图按底边定位，过高会裁顶部，高度未知则从预留区顶部按原尺寸绘制。
//
// 卡片圆角、内边距和行圆角由 fcitx_card_inset 推导，`user_radius` 的含义见那里。
//
// `style` 是用户的候选窗样式（见 FcitxCandidateStyle）。不透明度低于 100% 时卡片底下的阴影被擦掉，好让桌面透出来；同时打开 classicui 的 EnableBlur，并给出卡片形状的 BlurMask 和到卡片外沿的 BlurMargin：KWin 据此模糊卡片后面的内容（接近 macOS 半透明卡片的观感），其他合成器忽略这几个键，较旧的 Fcitx5 不认 BlurMask 时按 BlurMargin 模糊整个矩形。
inline FcitxThemeFiles fcitx_candidate_theme_files(const CandidateColors &colors, bool dark,
                                                   const std::optional<FcitxThemeOverlay> &overlay = std::nullopt,
                                                   const std::optional<double> &corner_radius = std::nullopt,
                                                   const std::optional<FcitxThemeLogo> &logo = std::nullopt,
                                                   bool user_radius = false,
                                                   const std::optional<FcitxCanvasPanel> &canvas_panel = std::nullopt,
                                                   const FcitxCandidateStyle &style = {},
                                                   const std::optional<FcitxThemeBackground> &background = std::nullopt) {
  using G = FcitxPanelGeometry;
  using M = FcitxMenuGeometry;
  const auto metrics = fcitx_panel_metrics(style);
  const float opacity = static_cast<float>(std::isfinite(style.opacity) ? std::clamp(style.opacity, 0.0, 1.0) : 1.0);
  const bool translucent = opacity < 1.0f && !canvas_panel;
  FcitxThemeFiles files;
  files.images.reserve(colors.selected ? 13 : 11);
  const auto surface = colors.background.value_or(0xffffffu);
  const auto text = colors.text.value_or(contrasting_color(surface).value_or(0));
  const auto selected_text = colors.selected_text.value_or(text);
  const auto highlight = colors.selected ? fcitx_theme_color(*colors.selected) : fcitx_theme_color(surface, true);
  const int border_width = colors.border ? std::max(0, colors.border_width) : 0;
  const auto border = border_width > 0 ? fcitx_theme_color(*colors.border) : fcitx_theme_color(surface, true);
  // 装饰图的裁剪边距止于描边内侧，没有描边时也留出发丝线的 1 px。
  const int inset = std::max(1, border_width);
  // 卡片圆角、内容到卡片外沿的距离和行圆角（fcitx_card_inset）。
  const auto card_inset = fcitx_card_inset(colors, style, corner_radius, user_radius);
  const int radius = card_inset.radius;
  const int pad = card_inset.pad;
  const int item_radius = card_inset.item_radius;
  const int band = overlay ? std::max(0, overlay->band) : 0;

  // The card: the corner slices hold the rounded corners and the part of the shadow that varies along the edge, so the stretched middle slices are exact. A decoration's band is the transparent top of the top slices.
  const int shadow_top = metrics.shadow_top + band;
  const int content_left = metrics.shadow_left + pad;
  const int logo_top = shadow_top + pad + metrics.item_vertical;
  const int lead = logo ? metrics.logo_side + metrics.logo_gap : 0;
  // 固定切片必须容下整条描边，否则边框色会进入可拉伸的中心，被拉成整片卡片；左上切片在有关标志时还要容下标志。
  const int edge = std::max(radius, border_width);
  const int slice_left = std::max(metrics.shadow_left + edge, logo ? content_left + metrics.logo_side : 0);
  const int slice_right = metrics.shadow_right + edge;
  const int slice_top = std::max(shadow_top + metrics.shadow_offset + edge, logo ? logo_top + metrics.logo_side : 0);
  const int slice_bottom = metrics.shadow_bottom + edge;
  const int panel_width = slice_left + 2 + slice_right;
  const int panel_height = slice_top + 2 + slice_bottom;
  const FcitxRect card{static_cast<double>(metrics.shadow_left), static_cast<double>(shadow_top),
                       static_cast<double>(panel_width - metrics.shadow_right),
                       static_cast<double>(panel_height - metrics.shadow_bottom)};
  const auto panel = fcitx_add_shape(files.images, panel_width, panel_height, [&](FcitxCanvas &canvas) {
    const FcitxRect shadow{card.left, card.top + metrics.shadow_offset, card.right, card.bottom + metrics.shadow_offset};
    fcitx_drop_shadow(canvas, shadow, radius, metrics.shadow_sigma, G::shadow_alpha);
    if (translucent) fcitx_clear_rounded(canvas, card, radius);
    fcitx_fill_rounded(canvas, card, radius, surface, opacity);
    if (border_width > 0) fcitx_stroke_rounded(canvas, card, radius, border_width, *colors.border, opacity);
    canvas.clear_above(band);
    if (logo) canvas.draw(canvas.scale() == 1 ? logo->one : logo->two, content_left, logo_top);
  });
  // KWin 按这张图的 alpha 决定模糊哪里：与卡片同一套九宫格，只有卡片本身不透明。
  std::string blur_mask;
  if (translucent)
    blur_mask = fcitx_add_shape(files.images, panel_width, panel_height, [&](FcitxCanvas &canvas) {
      fcitx_fill_rounded(canvas, card, radius, 0x000000u);
    });
  const int highlight_width = metrics.item_left + 2 + metrics.item_right;
  const int highlight_height = 2 * metrics.item_vertical + 2;
  std::string highlight_image;
  if (colors.selected)
    highlight_image = fcitx_add_shape(files.images, highlight_width, highlight_height, [&](FcitxCanvas &canvas) {
      fcitx_fill_rounded(canvas, {0, 0, static_cast<double>(highlight_width), static_cast<double>(highlight_height)},
                         item_radius, *colors.selected);
    });
  // The design's ‹ › page buttons in the card's header, drawn in its secondary colour, which is the number slot (THEME_CONTRACT: secondary always equals number). The classic UI draws page buttons only when both images load, at the right edge of the content, dimming the one with no page to go to. PageButtonAlignment=Top (Fcitx5 releases since April 2023) lifts them to the top row, where the preedit sits as the design's header does; older releases such as the 5.0.21 the tests run against ignore the key and draw them at the bottom. With no click margin the whole image is the click target, as the design's glyph with its padding is. 关掉 show_candidate_page_number 时不生成这两张图，classicui 就不画翻页按钮。
  std::string page_prev;
  std::string page_next;
  if (style.page_arrows) {
    const auto page_color = colors.number.value_or(text);
    const double unit = metrics.page_button / static_cast<double>(G::page_button);
    page_prev = fcitx_add_shape(files.images, metrics.page_button, metrics.page_button, [&](FcitxCanvas &canvas) {
      fcitx_stroke_polyline(canvas, {{9.5 * unit, 4.5 * unit}, {6 * unit, 8 * unit}, {9.5 * unit, 11.5 * unit}},
                            1.5 * unit, page_color);
    });
    page_next = fcitx_add_shape(files.images, metrics.page_button, metrics.page_button, [&](FcitxCanvas &canvas) {
      fcitx_stroke_polyline(canvas, {{6.5 * unit, 4.5 * unit}, {10 * unit, 8 * unit}, {6.5 * unit, 11.5 * unit}},
                            1.5 * unit, page_color);
    });
  }

  const auto menu = fcitx_menu_palette(dark, colors.menu);
  const int menu_size = 2 * M::radius + 2;
  const auto menu_background = fcitx_add_shape(files.images, menu_size, menu_size, [&](FcitxCanvas &canvas) {
    const FcitxRect whole{0, 0, static_cast<double>(menu_size), static_cast<double>(menu_size)};
    fcitx_fill_rounded(canvas, whole, M::radius, menu.background);
    if (menu.ring_stroke.alpha > 0)
      fcitx_stroke_rounded(canvas, whole, M::radius, 1.0, menu.ring_stroke.rgb, menu.ring_stroke.alpha / 255.0f);
  });
  const int menu_item_width = 2 * M::item_horizontal + 2;
  const int menu_item_height = 2 * M::item_vertical + 2;
  const auto menu_highlight = fcitx_add_shape(files.images, menu_item_width, menu_item_height, [&](FcitxCanvas &canvas) {
    fcitx_fill_rounded(canvas, {0, 0, static_cast<double>(menu_item_width), static_cast<double>(menu_item_height)},
                       M::item_radius, menu.hover);
  });
  const int separator_height = 2 * M::separator_gap + 1;
  const auto menu_separator = fcitx_add_shape(files.images, 2, separator_height, [&](FcitxCanvas &canvas) {
    fcitx_fill_rounded(canvas, {0, static_cast<double>(M::separator_gap), 2, static_cast<double>(M::separator_gap + 1)}, 0,
                       menu.separator);
  });
  // Without these the classic UI paints a white square for a checked item and for a sub-menu arrow.
  const auto menu_check = fcitx_add_shape(files.images, M::check_width, M::glyph_size, [&](FcitxCanvas &canvas) {
    fcitx_stroke_polyline(canvas, {{3.5, 8.5}, {6.5, 11.5}, {12.5, 4.5}}, 1.75, menu.text);
  });
  const auto menu_arrow = fcitx_add_shape(files.images, M::glyph_size, M::glyph_size, [&](FcitxCanvas &canvas) {
    fcitx_stroke_polyline(canvas, {{6.5, 4.5}, {10, 8}, {6.5, 11.5}}, 1.5, menu.text);
  });

  std::string decoration;
  if (overlay) {
    // The card's padding, where the content starts inside the outline: the image's bottom edge is this far below the card's top edge, and its side this far in from the card's side, as Windows places it with pad_y and pad_x.
    // With Top gravity the classic UI measures the offset down from the panel's top edge. An image of known height ends `pad` below the card's top (a negative offset is cut by the clip margin below); one of unknown height starts at the band's top.
    const int offset = overlay->height ? shadow_top + pad - *overlay->height : metrics.shadow_top;
    // Measured from the gravity's edge: `pad` inside the card on the side the skin aligns to, none when centred (the card is centred in the panel, its shadow margins being equal).
    const char *gravity = overlay->align == CandidateSkinAlign::left     ? "Top Left"
                          : overlay->align == CandidateSkinAlign::center ? "Top Center"
                                                                         : "Top Right";
    const int offset_x = overlay->align == CandidateSkinAlign::left     ? metrics.shadow_left + pad
                         : overlay->align == CandidateSkinAlign::center ? 0
                                                                        : metrics.shadow_right + pad;
    // The image may cover the band and the card down to just inside its outline, never the shadow around them.
    decoration = "Overlay=" + overlay->file + "\nGravity=" + gravity + "\nOverlayOffsetX=" +
                 std::to_string(offset_x) + "\nOverlayOffsetY=" + std::to_string(offset) +
                 "\nHideOverlayIfOversize=False\n\n[InputPanel/Background/OverlayClipMargin]\n" +
                 fcitx_margin(metrics.shadow_left + inset, metrics.shadow_right + inset, metrics.shadow_top,
                              metrics.shadow_bottom + inset);
    decoration.pop_back();
  } else if (background && !canvas_panel) {
    // 皮肤背景图：classicui 的背景只有一张叠图，没有装饰图时用它画背景。叠图居中贴在卡片中心（上下阴影边距不等，所以向上挪半个差值），裁到卡片里面：四边各收进描边宽度和圆角处的弧高（r·(1−1/√2)），矩形裁剪才不会让图片从圆角伸出去。
    const int corner = std::max(border_width, static_cast<int>(std::ceil(radius * (1.0 - 1.0 / std::sqrt(2.0)))));
    decoration = "Overlay=" + background->file + "\nGravity=Center\nOverlayOffsetX=" +
                 std::to_string((metrics.shadow_left - metrics.shadow_right) / 2) + "\nOverlayOffsetY=" +
                 std::to_string((shadow_top - metrics.shadow_bottom) / 2) +
                 "\nHideOverlayIfOversize=False\n\n[InputPanel/Background/OverlayClipMargin]\n" +
                 fcitx_margin(metrics.shadow_left + corner, metrics.shadow_right + corner, shadow_top + corner,
                              metrics.shadow_bottom + corner);
    decoration.pop_back();
  }
  std::ostringstream conf;
  // 文件必须以分组开头，不能有任何顶层键。在 GNOME Wayland 这类不让输入法自己弹窗的桌面上，候选窗由程序里的 fcitx5-gtk 模块按同一个主题绘制（客户端输入面板），它用 GKeyFile 读 theme.conf，遇到顶层键会整份拒绝并退回 default 主题，颜色和装饰图就都没了。所以经典界面加载 @2x 图要的 SupportedScale 不写成顶层的 `SupportedScale=2`，而写成 `[SupportedScale]` 分组下的 `Value=2`：这是 Fcitx5 把它改成可选值之后（上游 fcitx/fcitx5#1695，5.1.23 之后的版本）的写法，GKeyFile 也能读。5.1.22 及更早的版本不认这个键，5.1.23 只认顶层写法，读不出这个分组时退回 1，都只是不加载 @2x 图。
  conf << "[Metadata]\n"
          "Name=MSIME\n"
          "Version=1\n"
          "Author=MSIME\n"
          "Description=Generated from the MSIME candidate settings; edits are replaced when they change\n"
          "ScaleWithDPI=True\n\n"
          "[SupportedScale]\n"
          "Value=2\n\n"
          "[InputPanel]\n"
       << "NormalColor=" << fcitx_theme_color(text) << "\n"
       << "HighlightCandidateColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightBackgroundColor=" << highlight << "\n";
  if (colors.number) conf << "CandidateLabelColor=" << fcitx_theme_color(*colors.number) << "\n";
  if (colors.selected_number || colors.number)
    conf << "HighlightCandidateLabelColor=" << fcitx_theme_color(colors.selected_number.value_or(selected_text)) << "\n";
  // 序号按候选字号的 80% 画（macOS 的 0.8 倍）；Fcitx5 5.1.18 之前的版本不认这个键，序号与候选一样大。
  conf << "LabelTextSizeFactor=" << G::label_size_percent << "\n";
  if (translucent) conf << "EnableBlur=True\nBlurMask=" << blur_mask << "\n";
  else conf << "EnableBlur=False\n";
  conf << "FullWidthHighlight=True\n"
          "PageButtonAlignment=Top\n\n";
  if (translucent)
    conf << "[InputPanel/BlurMargin]\n"
         << fcitx_margin(metrics.shadow_left, metrics.shadow_right, shadow_top, metrics.shadow_bottom);
  conf << "[InputPanel/Background]\n"
       << "Image=" << (canvas_panel ? canvas_panel->file : panel) << "\n"
       << "Color=" << fcitx_theme_color(surface) << "\n"
       << "BorderColor=" << border << "\n"
       << "BorderWidth=" << border_width << "\n" << decoration << "\n"
       << "[InputPanel/Background/Margin]\n"
       << (canvas_panel ? fcitx_margin(canvas_panel->margin[3], canvas_panel->margin[1], canvas_panel->margin[0], canvas_panel->margin[2])
                        : fcitx_margin(slice_left, slice_right, slice_top, slice_bottom))
       << "[InputPanel/ShadowMargin]\n"
       << (canvas_panel ? fcitx_margin(canvas_panel->outer[3], canvas_panel->outer[1], canvas_panel->outer[0], canvas_panel->outer[2])
                        : fcitx_margin(metrics.shadow_left, metrics.shadow_right, shadow_top, metrics.shadow_bottom))
       << "[InputPanel/ContentMargin]\n"
       << (canvas_panel ? fcitx_margin(canvas_panel->content[3] + canvas_panel->outer[3], canvas_panel->content[1] + canvas_panel->outer[1],
                                      canvas_panel->content[0] + canvas_panel->outer[0], canvas_panel->content[2] + canvas_panel->outer[2])
                        : fcitx_margin(content_left + lead, metrics.shadow_right + pad, shadow_top + pad,
                                       metrics.shadow_bottom + pad))
       << "[InputPanel/TextMargin]\n"
       << fcitx_margin(metrics.item_left, metrics.item_right, metrics.item_vertical, metrics.item_vertical)
       << "[InputPanel/Highlight]\n";
  if (!highlight_image.empty()) conf << "Image=" << highlight_image << "\n";
  conf << "Color=" << highlight << "\n\n"
       << "[InputPanel/Highlight/Margin]\n"
       << fcitx_margin(metrics.item_left, metrics.item_right, metrics.item_vertical, metrics.item_vertical);
  if (style.page_arrows)
    conf << "[InputPanel/PrevPage]\n"
         << "Image=" << page_prev << "\n\n"
         << "[InputPanel/PrevPage/ClickMargin]\n" << fcitx_margin(0, 0, 0, 0)
         << "[InputPanel/NextPage]\n"
         << "Image=" << page_next << "\n\n"
         << "[InputPanel/NextPage/ClickMargin]\n" << fcitx_margin(0, 0, 0, 0);
  conf << "[Menu]\n"
       << "NormalColor=" << fcitx_theme_color(menu.text) << "\n"
       << "HighlightCandidateColor=" << fcitx_theme_color(menu.text) << "\n"
       << "Spacing=0\n\n"
       << "[Menu/Background]\n"
       << "Image=" << menu_background << "\n"
       << "Color=" << fcitx_theme_color(menu.background) << "\n"
       << "BorderColor=" << fcitx_theme_color(menu.ring) << "\n"
       << "BorderWidth=1\n\n"
       << "[Menu/Background/Margin]\n" << fcitx_margin(M::radius, M::radius, M::radius, M::radius)
       << "[Menu/ContentMargin]\n" << fcitx_margin(M::padding, M::padding, M::padding, M::padding)
       << "[Menu/Highlight]\n"
       << "Image=" << menu_highlight << "\n"
       << "Color=" << fcitx_theme_color(menu.hover) << "\n\n"
       << "[Menu/Highlight/Margin]\n"
       << fcitx_margin(M::item_horizontal, M::item_horizontal, M::item_vertical, M::item_vertical)
       << "[Menu/Separator]\n"
       << "Image=" << menu_separator << "\n"
       << "Color=" << fcitx_theme_color(menu.separator) << "\n\n"
       << "[Menu/CheckBox]\n"
       << "Image=" << menu_check << "\n\n"
       << "[Menu/SubMenu]\n"
       << "Image=" << menu_arrow << "\n\n"
       << "[Menu/TextMargin]\n"
       << fcitx_margin(M::item_horizontal, M::item_horizontal, M::item_vertical, M::item_vertical);
  files.conf = conf.str();
  files.conf.pop_back();
  if (canvas_panel)
    files.images.insert(files.images.end(), canvas_panel->images.begin(), canvas_panel->images.end());
  return files;
}

inline std::string fcitx_candidate_theme(const CandidateColors &colors, bool dark,
                                         const std::optional<FcitxThemeOverlay> &overlay = std::nullopt,
                                         const std::optional<double> &corner_radius = std::nullopt,
                                         const std::optional<FcitxThemeLogo> &logo = std::nullopt,
                                         bool user_radius = false, const FcitxCandidateStyle &style = {},
                                         const std::optional<FcitxThemeBackground> &background = std::nullopt) {
  return fcitx_candidate_theme_files(colors, dark, overlay, corner_radius, logo, user_radius, std::nullopt, style, background)
      .conf;
}

// Where Fcitx5 looks for a user theme: $XDG_DATA_HOME/fcitx5/themes/<name>/theme.conf. A relative XDG value is ignored, as the specification requires.
inline std::optional<std::filesystem::path> fcitx_theme_file(const char *xdg_data_home, const char *home) {
  std::filesystem::path base;
  if (xdg_data_home && std::filesystem::path(xdg_data_home).is_absolute())
    base = xdg_data_home;
  else if (home && std::filesystem::path(home).is_absolute())
    base = std::filesystem::path(home) / ".local/share";
  else
    return std::nullopt;
  return base / "fcitx5/themes" / std::string(kFcitxCandidateTheme) / "theme.conf";
}

// Replace the theme file atomically, leaving it untouched when it already holds the content. Returns whether the file now holds it.
inline bool write_fcitx_theme(const std::filesystem::path &file, const std::string &content) {
  const int descriptor = ::open(file.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor >= 0) {
    struct CloseOnExit {
      int descriptor;
      ~CloseOnExit() { ::close(descriptor); }
    } close_on_exit{descriptor};
    struct stat metadata {};
    if (::fstat(descriptor, &metadata) == 0 && S_ISREG(metadata.st_mode)) {
      std::string existing(content.size() + 1, '\0');
      ssize_t count = 0;
      for (;;) {
        count = ::read(descriptor, existing.data(), existing.size());
        if (count < 0 && errno == EINTR) continue;
        break;
      }
      if (count >= 0 && count == static_cast<ssize_t>(content.size()) &&
          existing.compare(0, content.size(), content) == 0)
        return true;
    }
  }
  return write_candidate_file_atomically(file, content);
}

// Copies of decoration images in the theme directory are named decoration-<content hash><extension>: a changed image gets a new name, so theme.conf changes with it and the classic UI loads the new picture rather than one it already holds under the old name.
inline constexpr std::string_view kFcitxOverlayPrefix = "decoration-";

// The largest decoration image copied, the same limit the shared layer puts on any skin asset (skin::catalog::MAX_RESOURCE_BYTES).
inline constexpr std::uintmax_t kFcitxOverlayMaxBytes = 8u * 1024u * 1024u;

// 解码边长上限与共享层 `skin::catalog::MAX_IMAGE_SIDE` 一致，拒绝仅靠文件头声明巨幅尺寸的图片。
inline constexpr int kFcitxOverlayMaxSide = 2048;

// 不解码读取 PNG 尺寸：签名、IHDR 和两边的 1..kFcitxOverlayMaxSide 范围都有效才接受；其他格式、截断或超限返回 `nullopt`。
struct FcitxPngSize {
  int width;
  int height;
};

inline std::optional<FcitxPngSize> fcitx_png_size(const std::string &bytes) {
  static constexpr char signature[] = "\x89PNG\r\n\x1a\n";
  if (bytes.size() < 24 || bytes.compare(0, 8, signature, 8) != 0 || bytes.compare(12, 4, "IHDR") != 0)
    return std::nullopt;
  const auto side = [&bytes](std::size_t at) {
    std::uint32_t value = 0;
    for (std::size_t index = at; index < at + 4; ++index) value = (value << 8) | static_cast<unsigned char>(bytes[index]);
    return value > 0 && value <= static_cast<std::uint32_t>(kFcitxOverlayMaxSide) ? static_cast<int>(value) : 0;
  };
  const int width = side(16);
  const int height = side(20);
  if (width == 0 || height == 0) return std::nullopt;
  return FcitxPngSize{width, height};
}

// 只需宽度的调用方从 PNG 头取值，供模式提示预留装饰宽度。
inline std::optional<int> fcitx_png_width(const std::string &bytes) {
  const auto size = fcitx_png_size(bytes);
  return size ? std::optional<int>(size->width) : std::nullopt;
}

// PNG 头的高度用于无法缩放时的原图定位，其他格式不提供高度。
inline std::optional<int> fcitx_png_height(const std::string &bytes) {
  const auto size = fcitx_png_size(bytes);
  return size ? std::optional<int>(size->height) : std::nullopt;
}

// Copy the decoration image into the theme directory, where the classic UI looks for an overlay (themes/<theme>/<Overlay>), with the same policy as theme.conf: atomically, and only when the copy does not already hold the same bytes. Only the image types a skin package may ship are taken, keeping their extension, which is how the classic UI picks a loader. Nothing is staged for a file that is missing, empty or over kFcitxOverlayMaxBytes.
//
// 有 `scale` 且是文件头可读、尺寸在上限内的 PNG 时，副本是缩放到皮肤声明的 width_dip 的图（@2x 副本是它的两倍），如果那样会超过预留高度加 `padding`（classicui 在卡片上方留给装饰图的空间），就继续等比缩小，保证整张图都能画出来；尺寸超限、缩放失败以及非 PNG 都退回原图，装饰不丢，只是按原尺寸放置。
inline std::optional<FcitxThemeOverlay> stage_fcitx_overlay(const std::filesystem::path &directory,
                                                            const CandidateSkinDecoration &decoration, int padding,
                                                            const FcitxOverlayScaler &scale = {}) {
  const std::filesystem::path source(decoration.image);
  auto extension = source.extension().string();
  std::transform(extension.begin(), extension.end(), extension.begin(),
                 [](unsigned char value) { return static_cast<char>(std::tolower(value)); });
  static constexpr std::string_view kinds[] = {".png", ".jpg", ".jpeg", ".gif", ".webp",
                                               ".svg", ".bmp", ".ico",  ".avif"};
  if (std::find(std::begin(kinds), std::end(kinds), extension) == std::end(kinds)) return std::nullopt;
  std::error_code error;
  if (!std::filesystem::is_regular_file(source, error)) return std::nullopt;
  const int descriptor = ::open(source.c_str(), O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK);
  if (descriptor < 0) return std::nullopt;
  struct CloseOnExit {
    int descriptor;
    ~CloseOnExit() { ::close(descriptor); }
  } close_on_exit{descriptor};
  struct stat metadata {};
  if (::fstat(descriptor, &metadata) != 0 || !S_ISREG(metadata.st_mode) || metadata.st_nlink != 1)
    return std::nullopt;
  std::string bytes;
  bytes.reserve(std::min<std::uintmax_t>(metadata.st_size, kFcitxOverlayMaxBytes));
  std::array<char, 64 * 1024> buffer{};
  for (;;) {
    const ssize_t count = ::read(descriptor, buffer.data(), buffer.size());
    if (count > 0) {
      const auto size = static_cast<std::size_t>(count);
      if (size > kFcitxOverlayMaxBytes || bytes.size() > kFcitxOverlayMaxBytes - size)
        return std::nullopt;
      bytes.append(buffer.data(), size);
      continue;
    }
    if (count == 0) break;
    if (errno == EINTR) continue;
    return std::nullopt;
  }
  if (bytes.empty() || bytes.size() > kFcitxOverlayMaxBytes) return std::nullopt;
  const int width = static_cast<int>(std::lround(decoration.width_dip));
  if (scale && extension == ".png" && width > 0 && fcitx_png_size(bytes)) {
    const int room = static_cast<int>(std::ceil(decoration.top_dip)) + std::max(0, padding);
    auto one = scale(bytes, width, room);
    auto two = one ? scale(bytes, 2 * width, 2 * room) : std::nullopt;
    const auto height = one ? fcitx_png_height(*one) : std::nullopt;
    if (one && two && height) {
      auto file = std::string(kFcitxOverlayPrefix) + fcitx_content_hash(*one + *two) + ".png";
      if (!write_fcitx_theme(directory / fcitx_overlay_2x_name(file), *two) || !write_fcitx_theme(directory / file, *one))
        return std::nullopt;
      return FcitxThemeOverlay{std::move(file), static_cast<int>(std::ceil(decoration.top_dip)), height,
                               decoration.align};
    }
  }
  auto file = std::string(kFcitxOverlayPrefix) + fcitx_content_hash(bytes) + extension;
  if (!write_fcitx_theme(directory / file, bytes)) return std::nullopt;
  return FcitxThemeOverlay{std::move(file), static_cast<int>(std::ceil(decoration.top_dip)), fcitx_png_height(bytes),
                           decoration.align};
}

// Remove every file with `prefix` whose name is not in `keep`, so the theme directory holds only the images the current theme names.
inline void remove_stale_fcitx_files(const std::filesystem::path &directory, std::string_view prefix,
                                     const std::vector<std::string> &keep) {
  std::error_code error;
  std::vector<std::filesystem::path> stale;
  for (std::filesystem::directory_iterator entry(directory, error), end; !error && entry != end; entry.increment(error)) {
    const auto name = entry->path().filename().string();
    if (name.compare(0, prefix.size(), prefix) == 0 && std::find(keep.begin(), keep.end(), name) == keep.end())
      stale.push_back(entry->path());
  }
  for (const auto &file : stale) std::filesystem::remove(file, error);
}

// What the overlay depends on, read without opening the image: its path, size and modification time and the declared band. The host compares this instead of copying the image on every refresh; a changed image changes the stamp and is staged again.
inline std::string fcitx_overlay_stamp(const std::optional<CandidateSkinDecoration> &decoration) {
  if (!decoration) return {};
  std::error_code size_error;
  std::error_code time_error;
  const auto size = std::filesystem::file_size(decoration->image, size_error);
  const auto time = std::filesystem::last_write_time(decoration->image, time_error);
  std::ostringstream stamp;
  stamp << "\n# overlay " << decoration->image << ' ' << decoration->top_dip << ' ' << decoration->width_dip << ' '
        << static_cast<int>(decoration->align) << ' '
        << (size_error ? 0 : size) << ' ' << (time_error ? 0LL : static_cast<long long>(time.time_since_epoch().count())) << '\n';
  return stamp.str();
}

// Write the theme for these colours and decoration into `file`. The images are written before theme.conf so the theme never names a file that is not there, and the images of an earlier theme are removed once theme.conf no longer names them. A decoration that cannot be staged leaves the theme without it rather than without MSIME's colours; a shape that cannot be written leaves theme.conf unchanged. Returns whether theme.conf now holds the theme. 装饰图的预留高度和宽度与卡片的其他长度一样按 `style.scale` 缩放（macOS 的 StyledCandidateSkin 也这样做）。
inline bool write_fcitx_candidate_theme(const std::filesystem::path &file, const CandidateColors &colors, bool dark,
                                        const std::optional<CandidateSkinDecoration> &decoration,
                                        const std::optional<double> &corner_radius = std::nullopt,
                                        const std::optional<FcitxThemeLogo> &logo = std::nullopt,
                                        bool user_radius = false, const FcitxOverlayScaler &scale = {},
                                        const std::optional<FcitxCanvasPanel> &canvas_panel = std::nullopt,
                                        const FcitxCandidateStyle &style = {},
                                        const std::optional<CandidateSkinBackground> &background = std::nullopt,
                                        const FcitxBackgroundRenderer &render_background = {}) {
  const auto directory = file.parent_path();
  if (!prepare_candidate_directory(directory)) return false;
  std::optional<FcitxThemeOverlay> overlay;
  if (decoration && !canvas_panel) {
    auto scaled = *decoration;
    const double factor = fcitx_panel_metrics(style).scale;
    scaled.top_dip *= factor;
    scaled.width_dip *= factor;
    overlay = stage_fcitx_overlay(directory, scaled, fcitx_card_padding(colors, style, corner_radius, user_radius), scale);
  }
  // 背景图和装饰图共用 classicui 唯一的一张叠图，装饰图画得出来时让给它；背景图的不透明度再乘上用户的候选窗不透明度，和 macOS 一样。
  std::optional<FcitxThemeBackground> staged_background;
  std::vector<std::string> backgrounds;
  if (background && render_background && !overlay && !canvas_panel) {
    const auto [width, height] = fcitx_background_box(style);
    const double opacity = std::clamp(background->opacity, 0.0, 1.0) *
                           (std::isfinite(style.opacity) ? std::clamp(style.opacity, 0.0, 1.0) : 1.0);
    if (const auto rendered = render_background(*background, width, height, opacity)) {
      auto name = std::string(kFcitxBackgroundPrefix) + fcitx_content_hash(rendered->first + rendered->second) + ".png";
      if (!write_fcitx_theme(directory / fcitx_overlay_2x_name(name), rendered->second) ||
          !write_fcitx_theme(directory / name, rendered->first))
        return false;
      backgrounds = {name, fcitx_overlay_2x_name(name)};
      staged_background = FcitxThemeBackground{std::move(name)};
    }
  }
  const auto theme = fcitx_candidate_theme_files(colors, dark, overlay, corner_radius, logo, user_radius, canvas_panel, style,
                                                 staged_background);
  std::vector<std::string> shapes;
  fcitx_collect_shape_names(theme, shapes);
  for (const auto &image : theme.images) {
    if (!write_fcitx_theme(directory / image.file, image.bytes)) return false;
  }
  if (!write_fcitx_theme(file, theme.conf)) return false;
  // A resampled decoration's @2x twin is kept with it; an image copied as it is has none, and the name kept for it matches nothing.
  std::vector<std::string> overlays;
  if (overlay) overlays = {overlay->file, fcitx_overlay_2x_name(overlay->file)};
  remove_stale_fcitx_files(directory, kFcitxOverlayPrefix, overlays);
  remove_stale_fcitx_files(directory, kFcitxBackgroundPrefix, backgrounds);
  remove_stale_fcitx_files(directory, kFcitxShapePrefix, shapes);
  remove_stale_fcitx_files(directory, kFcitxCanvasPrefix, shapes);
  return true;
}

}  // namespace msime::linux_host
