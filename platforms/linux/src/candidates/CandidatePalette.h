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

// Linux 的平台 token，解析出的全局主题留空的槽位都画它：`system` 主题的整套调色板，以及基于 `system` 的自定义主题没设的槽位。取自设计稿的 Adwaita token（设计画布里的 tok('linux')）：白色或 #303030 的卡片，正文和次要文字是黑或白的 82% 和 55%，选中行是实心的蓝底配白字、序号 85% 白，细线是 10% 黑或 8% 白。选中底不用 GNOME 强调色 #3584E4，它上面的白字只有约 3.8:1、85% 的序号和释义约 3.2:1，到不了 WCAG AA；换成同一色系更深的 Adwaita Blue 5 #1A5FB4（`kLinuxSelection`），白字约 6.3:1、序号和释义约 5.0:1。IBus 的文字属性和 Fcitx5 classic UI 主题在这些槽位上都不带 alpha，所以半透明的 token 在这里先合成到它所画的底色上。
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
// 默认的选中行底色：Adwaita 调色板的 Blue 5，比强调色深，白字和 85% 白的序号、释义都到 4.5:1。强调色仍是 `kLinuxAccent`，选中竖条、预编辑光标、键位图高亮这些不承载正文的地方照旧用它。
inline constexpr std::uint32_t kLinuxSelection = 0x1A5FB4u;

inline CandidateNativePalette candidate_native_palette(bool dark) {
  const std::uint32_t surface = dark ? 0x303030u : 0xFFFFFFu;
  const std::uint32_t ink = dark ? 0xFFFFFFu : 0x000000u;
  return CandidateNativePalette{
      surface,
      dark ? 0xFFFFFFu : composite_color(ink, 0xD1, surface),
      composite_color(ink, 0x8C, surface),
      dark ? 0x78AEEDu : 0x1C71D8u,
      kLinuxSelection,
      0xFFFFFFu,
      composite_color(0xFFFFFFu, 0xD9, kLinuxSelection),
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
