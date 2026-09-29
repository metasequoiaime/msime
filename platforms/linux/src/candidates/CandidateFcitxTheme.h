#pragma once

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <optional>
#include <sstream>
#include <string>
#include <string_view>
#include <system_error>
#include <vector>

#include "CandidateColors.h"
#include "CandidatePalette.h"
#include "FcitxThemeImages.h"
#include "AtomicWrite.h"

namespace msime::linux_host {

// Fcitx5's classic UI draws the candidate list from a named theme. MSIME publishes its palette as a theme of its own, so the list looks the same as on IBus and Windows, while a theme the user picked in fcitx5-configtool is never replaced: only Fcitx5's stock themes, or MSIME's own, are taken over.
inline constexpr std::string_view kFcitxCandidateTheme = "msime";

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

// An installed skin's decoration as the theme draws it (see stage_fcitx_overlay): the name of its copy in the theme directory, the band reserved for it above the candidates, and the image's own height where this host can read it.
struct FcitxThemeOverlay {
  std::string file;
  int band = 0;
  std::optional<int> height;
};

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

// Generated images are named shape-<content hash>.png with an @2x copy beside each: changed colours give new names, so theme.conf changes with them and the classic UI loads the new pictures rather than ones it already holds under the old names.
inline constexpr std::string_view kFcitxShapePrefix = "shape-";

// The Linux design's candidate card (tok('linux')): a 10 px radius, 6 px padding inside a 1 px hairline, 6 px rounded items with 5 px 12 px 5 px 10 px padding, and a 0 6px 18px rgba(0,0,0,.22) shadow. The classic UI positions the panel by the shadow margin only on X11 and never scales the shadow away, so the shadow is drawn tighter than the design's (4 px down, sigma 5 instead of 6 px and 9) to keep the transparent margin, and with it the offset Wayland compositors place the panel at, small. Items are padded 6 px vertically instead of 5 because a nine-slice corner cannot be shorter than its 6 px radius; the item pitch stays the design's, which also had a 2 px gap between rows.
struct FcitxPanelGeometry {
  static constexpr int radius = 10;
  static constexpr int padding = 6;
  static constexpr int shadow_left = 12;
  static constexpr int shadow_right = 12;
  static constexpr int shadow_top = 8;
  static constexpr int shadow_bottom = 16;
  static constexpr int shadow_offset = 4;
  static constexpr double shadow_sigma = 5.0;
  static constexpr float shadow_alpha = 0.22f;
  static constexpr int item_radius = 6;
  static constexpr int item_left = 10;
  static constexpr int item_right = 12;
  static constexpr int item_vertical = 6;
  static constexpr int page_button = 16;
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
      over(outlined ? slots.border : std::nullopt, 0x000000u, dark ? 0x5C : 0x1A),
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
// A decoration is drawn as the background's overlay. Windows draws it above the card, trailing-aligned, in a band top_inset_dip tall that pushes the card down; the classic UI draws an overlay only inside the panel, so here the band is the top of the card itself, just inside the outline: the content margin grows by it and the image sits at the top right. The classic UI draws an overlay at its own pixel size and cannot scale it into the width_dip x top_inset_dip box the way Windows does, so an image of known height is centred in the band like Windows' contain, and one taller than the band is anchored to its bottom so that what does not fit is cut at the card's top edge instead of being drawn over the candidates.
inline FcitxThemeFiles fcitx_candidate_theme_files(const CandidateColors &colors, bool dark,
                                                   const std::optional<FcitxThemeOverlay> &overlay = std::nullopt) {
  using G = FcitxPanelGeometry;
  using M = FcitxMenuGeometry;
  FcitxThemeFiles files;
  const auto surface = colors.background.value_or(0xffffffu);
  const auto text = colors.text.value_or(contrasting_color(surface).value_or(0));
  const auto selected_text = colors.selected_text.value_or(text);
  const auto highlight = colors.selected ? fcitx_theme_color(*colors.selected) : fcitx_theme_color(surface, true);
  const int border_width = colors.border ? std::max(0, colors.border_width) : 0;
  const auto border = border_width > 0 ? fcitx_theme_color(*colors.border) : fcitx_theme_color(surface, true);
  // The content keeps the design's 1 px hairline plus 6 px padding even without a border, and grows with a wider one, so the highlight never covers the outline.
  const int inset = std::max(1, border_width);
  const int band = overlay ? std::max(0, overlay->band) : 0;

  // The card: the corner slices hold the rounded corners and the part of the shadow that varies along the edge, so the stretched middle slices are exact.
  const int slice_left = G::shadow_left + G::radius;
  const int slice_right = G::shadow_right + G::radius;
  const int slice_top = G::shadow_top + G::shadow_offset + G::radius;
  const int slice_bottom = G::shadow_bottom + G::radius;
  const int panel_width = slice_left + 2 + slice_right;
  const int panel_height = slice_top + 2 + slice_bottom;
  const FcitxRect card{G::shadow_left, G::shadow_top, static_cast<double>(panel_width - G::shadow_right),
                       static_cast<double>(panel_height - G::shadow_bottom)};
  const auto panel = fcitx_add_shape(files.images, panel_width, panel_height, [&](FcitxCanvas &canvas) {
    const FcitxRect shadow{card.left, card.top + G::shadow_offset, card.right, card.bottom + G::shadow_offset};
    fcitx_drop_shadow(canvas, shadow, G::radius, G::shadow_sigma, G::shadow_alpha);
    fcitx_fill_rounded(canvas, card, G::radius, surface);
    if (border_width > 0) fcitx_stroke_rounded(canvas, card, G::radius, border_width, *colors.border);
  });
  std::string highlight_image;
  if (colors.selected)
    highlight_image = fcitx_add_shape(files.images, G::item_left + 2 + G::item_right, 2 * G::item_vertical + 2,
                                      [&](FcitxCanvas &canvas) {
                                        fcitx_fill_rounded(canvas,
                                                           {0, 0, static_cast<double>(G::item_left + 2 + G::item_right),
                                                            static_cast<double>(2 * G::item_vertical + 2)},
                                                           G::item_radius, *colors.selected);
                                      });
  // The design's ‹ › page buttons in the card's header, drawn in its secondary colour, which is the number slot (THEME_CONTRACT: secondary always equals number). The classic UI draws page buttons only when both images load, at the right edge of the content, dimming the one with no page to go to. PageButtonAlignment=Top (Fcitx5 releases since April 2023) lifts them to the top row, where the preedit sits as the design's header does; older releases such as the 5.0.21 the tests run against ignore the key and draw them at the bottom. With no click margin the whole image is the click target, as the design's glyph with its padding is.
  const auto page_color = colors.number.value_or(text);
  const auto page_prev = fcitx_add_shape(files.images, G::page_button, G::page_button, [&](FcitxCanvas &canvas) {
    fcitx_stroke_polyline(canvas, {{9.5, 4.5}, {6, 8}, {9.5, 11.5}}, 1.5, page_color);
  });
  const auto page_next = fcitx_add_shape(files.images, G::page_button, G::page_button, [&](FcitxCanvas &canvas) {
    fcitx_stroke_polyline(canvas, {{6.5, 4.5}, {10, 8}, {6.5, 11.5}}, 1.5, page_color);
  });

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
    int offset = G::shadow_top + inset;
    if (overlay->height) offset += *overlay->height <= band ? (band - *overlay->height) / 2 : band - *overlay->height;
    decoration = "Overlay=" + overlay->file + "\nGravity=Top Right\nOverlayOffsetX=" +
                 std::to_string(G::shadow_right + inset) + "\nOverlayOffsetY=" + std::to_string(offset) +
                 "\nHideOverlayIfOversize=False\n\n[InputPanel/Background/OverlayClipMargin]\n" +
                 fcitx_margin(G::shadow_left + inset, G::shadow_right + inset, G::shadow_top + inset,
                              G::shadow_bottom + inset);
    decoration.pop_back();
  }
  std::ostringstream conf;
  conf << "SupportedScale=2\n\n"
          "[Metadata]\n"
          "Name=MSIME\n"
          "Version=1\n"
          "Author=MSIME\n"
          "Description=Generated from the MSIME candidate settings; edits are replaced when they change\n"
          "ScaleWithDPI=True\n\n"
          "[InputPanel]\n"
       << "NormalColor=" << fcitx_theme_color(text) << "\n"
       << "HighlightCandidateColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightBackgroundColor=" << highlight << "\n";
  if (colors.number) conf << "CandidateLabelColor=" << fcitx_theme_color(*colors.number) << "\n";
  if (colors.selected_number || colors.number)
    conf << "HighlightCandidateLabelColor=" << fcitx_theme_color(colors.selected_number.value_or(selected_text)) << "\n";
  conf << "EnableBlur=False\n"
          "FullWidthHighlight=True\n"
          "PageButtonAlignment=Top\n\n"
          "[InputPanel/Background]\n"
       << "Image=" << panel << "\n"
       << "Color=" << fcitx_theme_color(surface) << "\n"
       << "BorderColor=" << border << "\n"
       << "BorderWidth=" << border_width << "\n" << decoration << "\n"
       << "[InputPanel/Background/Margin]\n" << fcitx_margin(slice_left, slice_right, slice_top, slice_bottom)
       << "[InputPanel/ShadowMargin]\n"
       << fcitx_margin(G::shadow_left, G::shadow_right, G::shadow_top, G::shadow_bottom)
       << "[InputPanel/ContentMargin]\n"
       << fcitx_margin(G::shadow_left + inset + G::padding, G::shadow_right + inset + G::padding,
                       G::shadow_top + inset + band + G::padding, G::shadow_bottom + inset + G::padding)
       << "[InputPanel/TextMargin]\n" << fcitx_margin(G::item_left, G::item_right, G::item_vertical, G::item_vertical)
       << "[InputPanel/Highlight]\n";
  if (!highlight_image.empty()) conf << "Image=" << highlight_image << "\n";
  conf << "Color=" << highlight << "\n\n"
       << "[InputPanel/Highlight/Margin]\n" << fcitx_margin(G::item_left, G::item_right, G::item_vertical, G::item_vertical)
       << "[InputPanel/PrevPage]\n"
       << "Image=" << page_prev << "\n\n"
       << "[InputPanel/PrevPage/ClickMargin]\n" << fcitx_margin(0, 0, 0, 0)
       << "[InputPanel/NextPage]\n"
       << "Image=" << page_next << "\n\n"
       << "[InputPanel/NextPage/ClickMargin]\n" << fcitx_margin(0, 0, 0, 0)
       << "[Menu]\n"
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
  return files;
}

inline std::string fcitx_candidate_theme(const CandidateColors &colors, bool dark,
                                         const std::optional<FcitxThemeOverlay> &overlay = std::nullopt) {
  return fcitx_candidate_theme_files(colors, dark, overlay).conf;
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
  {
    std::ifstream current(file, std::ios::binary);
    if (current) {
      std::string existing(content.size() + 1, '\0');
      current.read(existing.data(), static_cast<std::streamsize>(existing.size()));
      const auto count = current.gcount();
      if (count == static_cast<std::streamsize>(content.size()) &&
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

// The height recorded in a PNG header, which is all this host reads of an image. Other formats are drawn without it.
inline std::optional<int> fcitx_png_height(const std::string &bytes) {
  static constexpr char signature[] = "\x89PNG\r\n\x1a\n";
  if (bytes.size() < 24 || bytes.compare(0, 8, signature, 8) != 0 || bytes.compare(12, 4, "IHDR") != 0)
    return std::nullopt;
  std::uint32_t height = 0;
  for (std::size_t index = 20; index < 24; ++index) height = (height << 8) | static_cast<unsigned char>(bytes[index]);
  if (height == 0 || height > 0x7fffffffu) return std::nullopt;
  return static_cast<int>(height);
}

// Copy the decoration image into the theme directory, where the classic UI looks for an overlay (themes/<theme>/<Overlay>), with the same policy as theme.conf: atomically, and only when the copy does not already hold the same bytes. Only the image types a skin package may ship are taken, keeping their extension, which is how the classic UI picks a loader. Nothing is staged for a file that is missing, empty or over kFcitxOverlayMaxBytes.
inline std::optional<FcitxThemeOverlay> stage_fcitx_overlay(const std::filesystem::path &directory,
                                                            const CandidateSkinDecoration &decoration) {
  const std::filesystem::path source(decoration.image);
  auto extension = source.extension().string();
  std::transform(extension.begin(), extension.end(), extension.begin(),
                 [](unsigned char value) { return static_cast<char>(std::tolower(value)); });
  static constexpr std::string_view kinds[] = {".png", ".jpg", ".jpeg", ".gif", ".webp",
                                               ".svg", ".bmp", ".ico",  ".avif"};
  if (std::find(std::begin(kinds), std::end(kinds), extension) == std::end(kinds)) return std::nullopt;
  std::error_code error;
  if (!std::filesystem::is_regular_file(source, error)) return std::nullopt;
  std::string bytes;
  {
    std::ifstream in(source, std::ios::binary);
    if (!in) return std::nullopt;
    std::vector<char> buffer(64 * 1024);
    while (in && bytes.size() <= kFcitxOverlayMaxBytes) {
      in.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
      bytes.append(buffer.data(), static_cast<std::size_t>(in.gcount()));
    }
    if (in.bad()) return std::nullopt;
  }
  if (bytes.empty() || bytes.size() > kFcitxOverlayMaxBytes) return std::nullopt;
  auto file = std::string(kFcitxOverlayPrefix) + fcitx_content_hash(bytes) + extension;
  if (!write_fcitx_theme(directory / file, bytes)) return std::nullopt;
  return FcitxThemeOverlay{std::move(file), static_cast<int>(std::ceil(decoration.top_dip)), fcitx_png_height(bytes)};
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
  stamp << "\n# overlay " << decoration->image << ' ' << decoration->top_dip << ' '
        << (size_error ? 0 : size) << ' ' << (time_error ? 0LL : static_cast<long long>(time.time_since_epoch().count())) << '\n';
  return stamp.str();
}

// Write the theme for these colours and decoration into `file`. The images are written before theme.conf so the theme never names a file that is not there, and the images of an earlier theme are removed once theme.conf no longer names them. A decoration that cannot be staged leaves the theme without it rather than without MSIME's colours; a shape that cannot be written leaves theme.conf unchanged. Returns whether theme.conf now holds the theme.
inline bool write_fcitx_candidate_theme(const std::filesystem::path &file, const CandidateColors &colors, bool dark,
                                        const std::optional<CandidateSkinDecoration> &decoration) {
  const auto directory = file.parent_path();
  std::error_code error;
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  const auto overlay = decoration ? stage_fcitx_overlay(directory, *decoration) : std::nullopt;
  const auto theme = fcitx_candidate_theme_files(colors, dark, overlay);
  std::vector<std::string> shapes;
  for (const auto &image : theme.images) {
    if (!write_fcitx_theme(directory / image.file, image.bytes)) return false;
    shapes.push_back(image.file);
  }
  if (!write_fcitx_theme(file, theme.conf)) return false;
  remove_stale_fcitx_files(directory, kFcitxOverlayPrefix, overlay ? std::vector<std::string>{overlay->file} : std::vector<std::string>{});
  remove_stale_fcitx_files(directory, kFcitxShapePrefix, shapes);
  return true;
}

}  // namespace msime::linux_host
