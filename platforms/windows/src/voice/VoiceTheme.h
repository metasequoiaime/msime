#pragma once
#include "../candidate/CandidatePalette.h"
#include <string_view>

namespace msime::windows {
enum class SurfaceThemeMode { Dark, Light, System };

inline SurfaceThemeMode surface_theme_mode(std::string_view surface,
                                           std::string_view global) {
  if (surface == "light")
    return SurfaceThemeMode::Light;
  if (surface == "dark")
    return SurfaceThemeMode::Dark;
  if (global == "light")
    return SurfaceThemeMode::Light;
  if (global == "system")
    return SurfaceThemeMode::System;
  return SurfaceThemeMode::Dark;
}

inline bool surface_theme_is_light(SurfaceThemeMode mode, bool system_dark) {
  return mode == SurfaceThemeMode::Light ||
         (mode == SurfaceThemeMode::System && !system_dark);
}

// Match the shared panel: an explicit surface wins, then the global theme,
// and only a global system preference consults Windows.
inline bool voice_theme_is_light(std::string_view surface,
                                 std::string_view global, bool system_dark) {
  return surface_theme_is_light(surface_theme_mode(surface, global),
                                system_dark);
}
// The voice overlay's colours, from the same theme palette the floating toolbar and the tray menu draw with, resolved in the overlay's own light/dark mode above: the panel is the surface and its outline the border, the waveform is the accent, the transcript and the cancel and confirm glyphs are the text, and the two action discs are the text at 12% so they read on any theme's surface.
struct VoiceOverlayColors {
  CandidateColor surface, border, wave, text, action;
  friend bool operator==(const VoiceOverlayColors &left,
                         const VoiceOverlayColors &right) {
    return left.surface == right.surface && left.border == right.border &&
           left.wave == right.wave && left.text == right.text &&
           left.action == right.action;
  }
  friend bool operator!=(const VoiceOverlayColors &left,
                         const VoiceOverlayColors &right) {
    return !(left == right);
  }
};
inline VoiceOverlayColors voice_overlay_colors(const CandidatePalette &palette) {
  return {palette.surface,
          palette.border,
          palette.accent,
          palette.text,
          {palette.text.r, palette.text.g, palette.text.b, palette.text.a * 0.12f}};
}
} // namespace msime::windows
