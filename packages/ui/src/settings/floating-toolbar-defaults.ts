import type { FloatingToolbarPreferences } from "./floating-toolbar-preferences";

/** Mirrors FloatingToolbarPreferences::default() in client-core. */
export const defaultFloatingToolbar: FloatingToolbarPreferences = {
  enabled: true,
  english_mode: true,
  input_scheme: true,
  fullwidth: true,
  punctuation: true,
  character_set: true,
  emoji: false,
  handwriting: false,
  screen_keyboard: false,
  voice: false,
  settings: true,
  auto_hide: false,
  auto_hide_delay: 5,
  shadow: true,
  scale_percent: 100,
  font_size: 24,
};
