#include "../../src/voice/VoiceTheme.h"
#include <cassert>
#include <string_view>

int main() {
  using msime::windows::SurfaceThemeMode;
  using msime::windows::surface_theme_is_light;
  using msime::windows::surface_theme_mode;
  using msime::windows::voice_theme_is_light;
  for (bool system_dark : {false, true}) {
    for (std::string_view global : {"dark", "light", "system"}) {
      assert(voice_theme_is_light("light", global, system_dark));
      assert(!voice_theme_is_light("dark", global, system_dark));
      assert(voice_theme_is_light("follow", global, system_dark) ==
             (global == "light" || (global == "system" && !system_dark)));
    }
  }
  assert(surface_theme_mode("light", "system") == SurfaceThemeMode::Light);
  assert(surface_theme_mode("dark", "system") == SurfaceThemeMode::Dark);
  assert(surface_theme_mode("follow", "light") == SurfaceThemeMode::Light);
  assert(surface_theme_mode("follow", "dark") == SurfaceThemeMode::Dark);
  assert(surface_theme_mode("follow", "system") == SurfaceThemeMode::System);
  assert(surface_theme_is_light(SurfaceThemeMode::System, false));
  assert(!surface_theme_is_light(SurfaceThemeMode::System, true));

  // The overlay draws the theme palette of its mode rather than colours of its own: surface, border, accent for the waveform and text for the transcript and glyphs, with the action discs a faint wash of the text.
  using msime::windows::candidate_native_palette;
  using msime::windows::candidate_palette;
  using msime::windows::candidate_rgb;
  using msime::windows::voice_overlay_colors;
  for (bool dark : {false, true}) {
    const auto palette = candidate_native_palette(dark);
    const auto colors = voice_overlay_colors(palette);
    assert(colors.surface == palette.surface && colors.border == palette.border &&
           colors.wave == palette.accent && colors.text == palette.text);
    assert(colors.action.r == palette.text.r && colors.action.g == palette.text.g &&
           colors.action.b == palette.text.b &&
           colors.action.a == palette.text.a * 0.12f);
  }
  assert(voice_overlay_colors(candidate_native_palette(false)) !=
         voice_overlay_colors(candidate_native_palette(true)));
  // A theme's slots reach the overlay, so a skin that recolours the card recolours it too.
  msime::windows::CandidatePaletteOverrides themed;
  themed.surface = "#102030";
  themed.accent = "#ff8800";
  themed.text = "#fafafa";
  themed.border = "#405060";
  const auto colors = voice_overlay_colors(candidate_palette(themed));
  assert(colors.surface == candidate_rgb(0x102030) &&
         colors.wave == candidate_rgb(0xFF8800) &&
         colors.text == candidate_rgb(0xFAFAFA) &&
         colors.border == candidate_rgb(0x405060));
}
