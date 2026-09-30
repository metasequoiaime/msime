#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace msime::linux_host {

// Source-over of one colour at the given alpha on an opaque background.
inline std::uint32_t composite_color(std::uint32_t color, std::uint8_t alpha, std::uint32_t background) {
  std::uint32_t result = 0;
  for (const int shift : {16, 8, 0}) {
    const auto top = (color >> shift) & 0xffu;
    const auto bottom = (background >> shift) & 0xffu;
    result |= ((top * alpha + bottom * (255u - alpha) + 127u) / 255u) << shift;
  }
  return result;
}

// The Linux platform tokens, drawn wherever the resolved global theme leaves a slot null: the whole palette for the `system` theme, and the unset slots of a custom theme over `system`. They are the design's Adwaita tokens (tok('linux') in the design canvas): a white or #303030 card, text at 82% and secondary text at 55% of black or white, the GNOME accent #3584E4 as a solid selection with white text and numbers at 85%, and a hairline at 10% black or 8% white. Neither IBus text attributes nor the Fcitx5 classic UI theme carry alpha on these slots, so each translucent token is composited here over the surface it is drawn on.
struct CandidateNativePalette {
  std::uint32_t surface;
  std::uint32_t text;
  std::uint32_t number;
  // The accent as a text colour on the card (pinned candidates): GNOME's darker accent text on light, its lighter one on dark, so it stays readable where the fill accent would not.
  std::uint32_t accent;
  std::uint32_t selected;
  std::uint32_t selected_text;
  std::uint32_t selected_number;
  std::uint32_t border;
};

inline constexpr std::uint32_t kLinuxAccent = 0x3584E4u;

inline CandidateNativePalette candidate_native_palette(bool dark) {
  const std::uint32_t surface = dark ? 0x303030u : 0xFFFFFFu;
  const std::uint32_t ink = dark ? 0xFFFFFFu : 0x000000u;
  return CandidateNativePalette{
      surface,
      dark ? 0xFFFFFFu : composite_color(ink, 0xD1, surface),
      composite_color(ink, 0x8C, surface),
      dark ? 0x78AEEDu : 0x1C71D8u,
      kLinuxAccent,
      0xFFFFFFu,
      composite_color(0xFFFFFFu, 0xD9, kLinuxAccent),
      composite_color(ink, dark ? 0x14 : 0x1A, surface),
  };
}

// The palette of a floating surface MSIME draws itself (the mode badge, the voice overlay), taken from the resolved theme's candidate palette as THEME_CONTRACT §3 derives the floating toolbar from it: surface for the plate, text for glyphs, accent for highlights and border for the outline, all opaque. No border means the theme draws none.
struct FloatingSurfaceColors {
  std::uint32_t surface;
  std::uint32_t text;
  std::uint32_t accent;
  std::optional<std::uint32_t> border;
};

// IBus auxiliary text has no native caret geometry. When the displayed
// candidate preedit is the Engine's ASCII editing text, a plain-text marker is
// the least surprising Linux equivalent of the Windows candidate caret. Do
// not guess when the host has transformed the displayed text or the offset is
// invalid.
inline std::string candidate_preedit_with_caret(std::string_view preedit,
                                                std::string_view editing,
                                                std::size_t caret) {
  std::string result(preedit);
  if (preedit.empty() || preedit != editing || caret > preedit.size())
    return result;
  result.insert(caret, "|");
  return result;
}

} // namespace msime::linux_host
