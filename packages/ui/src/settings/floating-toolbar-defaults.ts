import type { FloatingToolbarPreferences } from "../index";

/** Mirrors FloatingToolbarPreferences::default() in client-core. */
export const defaultFloatingToolbar: FloatingToolbarPreferences = {
  enabled: true,
  english_mode: true,
  fullwidth: true,
  punctuation: true,
  character_set: true,
  emoji: false,
  handwriting: false,
  screen_keyboard: false,
  voice: false,
  settings: true,
  scale_percent: 100,
  font_size: 24,
};
