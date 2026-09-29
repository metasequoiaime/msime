import type { Preferences } from "../index";
import { defaultFrequency } from "./frequency-section";
import { defaultFuzzyPinyin } from "./fuzzy-pinyin-section";
import { defaultKeybindings } from "./keybinding-defaults";
import { defaultLocalModes } from "./local-modes-section";
import { defaultMixedInput } from "./mixed-input-section";
import { defaultNavigation } from "./navigation-section";
import { defaultWordCharacter } from "./word-character-section";

export type SettingsInputPreferencesSource = Pick<
  Preferences,
  | "word_character"
  | "keybindings"
  | "frequency"
  | "mixed_input"
  | "fuzzy_pinyin"
  | "local_modes"
  | "navigation"
  | "number_row_selection"
>;

export interface SettingsInputPreferences {
  wordCharacter: NonNullable<Preferences["word_character"]>;
  keybindings: NonNullable<Preferences["keybindings"]>;
  frequency: NonNullable<Preferences["frequency"]>;
  mixedInput: NonNullable<Preferences["mixed_input"]>;
  fuzzyPinyin: NonNullable<Preferences["fuzzy_pinyin"]>;
  localModes: NonNullable<Preferences["local_modes"]>;
  navigation: NonNullable<Preferences["navigation"]>;
  numberRowSelection: boolean;
}

/** Resolves input, shortcut, and local-mode preferences for the settings panels. */
export function settingsInputPreferences(
  draft?: SettingsInputPreferencesSource,
): SettingsInputPreferences {
  return {
    wordCharacter: draft?.word_character ?? defaultWordCharacter,
    keybindings: draft?.keybindings ?? defaultKeybindings,
    frequency: draft?.frequency ?? defaultFrequency,
    mixedInput: draft?.mixed_input ?? defaultMixedInput,
    fuzzyPinyin: draft?.fuzzy_pinyin ?? defaultFuzzyPinyin,
    localModes: draft?.local_modes ?? defaultLocalModes,
    navigation: draft?.navigation ?? defaultNavigation,
    numberRowSelection: draft?.number_row_selection ?? true,
  };
}
