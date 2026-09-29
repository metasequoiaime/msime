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

// The classic UI theme format has no label or accent colour and no hover state separate from the highlight, so the candidate number follows the text and the accent is not drawn; everything the format can carry comes from the same resolution IBus uses. Without a selected fill (the graphite skin) the highlight is transparent and the selected row is told apart by its text colour alone, as on Windows.
//
// A decoration is drawn as the background's overlay. Windows draws it above the card, trailing-aligned, in a band top_inset_dip tall that pushes the card down; the classic UI draws an overlay only inside the panel, so here the band is the top of the panel itself: the content margin grows by it and the image sits at the top right, clear of the outline. The classic UI draws an overlay at its own pixel size and cannot scale it into the width_dip x top_inset_dip box the way Windows does, so an image of known height is centred in the band like Windows' contain, and one taller than the band is anchored to its bottom so that what does not fit is cut at the panel's top edge instead of being drawn over the candidates.
inline std::string fcitx_candidate_theme(const CandidateColors &colors,
                                         const std::optional<FcitxThemeOverlay> &overlay = std::nullopt) {
  const auto surface = colors.background.value_or(0xffffffu);
  const auto text = colors.text.value_or(contrasting_color(surface).value_or(0));
  const auto selected_text = colors.selected_text.value_or(text);
  const auto highlight = colors.selected ? fcitx_theme_color(*colors.selected) : fcitx_theme_color(surface, true);
  // The classic UI clips the border to the smallest background margin and draws it inside that margin, so the margins grow with the width, and the content margin with them so the highlight never covers the outline. At the widths the skins use (0 or 1) both stay at 2 and the list keeps its layout.
  const int border_width = colors.border ? std::max(0, colors.border_width) : 0;
  const auto border = border_width > 0 ? fcitx_theme_color(*colors.border) : fcitx_theme_color(surface, true);
  const int edge_width = std::max(2, border_width + 1);
  const auto edge = std::to_string(edge_width);
  const auto margin = "Left=" + edge + "\nRight=" + edge + "\nTop=" + edge + "\nBottom=" + edge + "\n\n";
  const int band = overlay ? std::max(0, overlay->band) : 0;
  const auto content_margin = "Left=" + edge + "\nRight=" + edge + "\nTop=" + std::to_string(edge_width + band) +
                              "\nBottom=" + edge + "\n\n";
  std::string decoration;
  if (overlay) {
    int offset = edge_width;
    if (overlay->height) offset += *overlay->height <= band ? (band - *overlay->height) / 2 : band - *overlay->height;
    const auto clip = std::to_string(border_width);
    decoration = "Overlay=" + overlay->file + "\nGravity=Top Right\nOverlayOffsetX=" + clip +
                 "\nOverlayOffsetY=" + std::to_string(offset) + "\nHideOverlayIfOversize=False\n\n"
                 "[InputPanel/Background/OverlayClipMargin]\nLeft=" + clip + "\nRight=" + clip + "\nTop=" + clip +
                 "\nBottom=" + clip + "\n";
  }
  std::ostringstream conf;
  conf << "[Metadata]\n"
          "Name=MSIME\n"
          "Version=1\n"
          "Author=MSIME\n"
          "Description=Generated from the MSIME candidate settings; edits are replaced when they change\n"
          "ScaleWithDPI=True\n\n"
          "[InputPanel]\n"
       << "NormalColor=" << fcitx_theme_color(text) << "\n"
       << "HighlightCandidateColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightColor=" << fcitx_theme_color(selected_text) << "\n"
       << "HighlightBackgroundColor=" << highlight << "\n"
       << "EnableBlur=False\n"
          "FullWidthHighlight=True\n"
          "Spacing=0\n\n"
          "[InputPanel/Background]\n"
       << "Color=" << fcitx_theme_color(surface) << "\n"
       << "BorderColor=" << border << "\n"
       << "BorderWidth=" << border_width << "\n" << decoration << "\n"
       << "[InputPanel/Background/Margin]\n" << margin
       << "[InputPanel/ContentMargin]\n" << content_margin
       << "[InputPanel/TextMargin]\nLeft=6\nRight=6\nTop=4\nBottom=4\n\n"
          "[InputPanel/Highlight]\n"
       << "Color=" << highlight << "\n\n"
       << "[InputPanel/Highlight/Margin]\nLeft=6\nRight=6\nTop=4\nBottom=4\n\n"
          "[Menu]\n"
       << "NormalColor=" << fcitx_theme_color(text) << "\n"
       << "HighlightCandidateColor=" << fcitx_theme_color(selected_text) << "\n\n"
       << "[Menu/Background]\n"
       << "Color=" << fcitx_theme_color(surface) << "\n\n"
       << "[Menu/Highlight]\n"
       << "Color=" << highlight << "\n\n"
       << "[Menu/Separator]\n"
       << "Color=" << fcitx_theme_color(text) << "\n\n"
       << "[Menu/ContentMargin]\nLeft=2\nRight=2\nTop=2\nBottom=2\n\n"
          "[Menu/TextMargin]\nLeft=6\nRight=6\nTop=4\nBottom=4\n";
  return conf.str();
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
  std::error_code error;
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
  std::filesystem::create_directories(file.parent_path(), error);
  if (error) return false;
  auto staged = file;
  staged += ".new";
  {
    std::ofstream out(staged, std::ios::binary | std::ios::trunc);
    if (!(out << content) || !out.flush()) return false;
  }
  std::filesystem::rename(staged, file, error);
  if (error) {
    std::filesystem::remove(staged, error);
    return false;
  }
  return true;
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
  std::uint64_t hash = 14695981039346656037ull;
  for (const char value : bytes) {
    hash ^= static_cast<unsigned char>(value);
    hash *= 1099511628211ull;
  }
  char digest[17];
  std::snprintf(digest, sizeof digest, "%016llx", static_cast<unsigned long long>(hash));
  auto file = std::string(kFcitxOverlayPrefix) + digest + extension;
  if (!write_fcitx_theme(directory / file, bytes)) return std::nullopt;
  return FcitxThemeOverlay{std::move(file), static_cast<int>(std::ceil(decoration.top_dip)), fcitx_png_height(bytes)};
}

// Remove every decoration copy but `keep` (empty: all of them), so the theme directory holds only the image the current theme names.
inline void remove_stale_fcitx_overlays(const std::filesystem::path &directory, std::string_view keep) {
  std::error_code error;
  std::vector<std::filesystem::path> stale;
  for (std::filesystem::directory_iterator entry(directory, error), end; !error && entry != end; entry.increment(error)) {
    const auto name = entry->path().filename().string();
    if (name.compare(0, kFcitxOverlayPrefix.size(), kFcitxOverlayPrefix) == 0 && name != keep)
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

// Write the theme for these colours and decoration into `file`. The image is staged before theme.conf so the theme never names a copy that is not there, and the copies of an earlier skin are removed once theme.conf no longer names them. An image that cannot be staged leaves the theme without a decoration rather than without MSIME's colours. Returns whether theme.conf now holds the theme.
inline bool write_fcitx_candidate_theme(const std::filesystem::path &file, const CandidateColors &colors,
                                        const std::optional<CandidateSkinDecoration> &decoration) {
  const auto directory = file.parent_path();
  std::error_code error;
  std::filesystem::create_directories(directory, error);
  if (error) return false;
  const auto overlay = decoration ? stage_fcitx_overlay(directory, *decoration) : std::nullopt;
  if (!write_fcitx_theme(file, fcitx_candidate_theme(colors, overlay))) return false;
  remove_stale_fcitx_overlays(directory, overlay ? std::string_view(overlay->file) : std::string_view());
  return true;
}

}  // namespace msime::linux_host
