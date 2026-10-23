import { expect, test } from "vitest";
import { defaultMixedInput as publicDefault, settingsInputPreferences } from "@msime/ui";
import { defaultMixedInput as legacyDefault } from "../../../../packages/ui/src/settings/mixed-input-section";
import {
  defaultMixedInput,
  mixedInputControlState,
  mixedInputDefaultsFor,
} from "../../../../packages/ui/src/settings/mixed-input-preferences";

test("默认对象保留旧文档的四项且所有入口共用同一引用", () => {
  expect(defaultMixedInput).toEqual({
    english: true,
    minimum_prefix: 5,
    emoji: false,
    kaomoji: false,
  });
  expect(publicDefault).toBe(defaultMixedInput);
  expect(legacyDefault).toBe(defaultMixedInput);
  expect(settingsInputPreferences().mixedInput).toBe(defaultMixedInput);
});

test("文档缺 mixed_input 时 emoji 混输按宿主平台取缺省，与 client-core 一致", () => {
  // Rust 的 `source_mixed_emoji_default` 在 Windows 和 macOS 上为开。
  for (const platform of ["macos", "windows"]) {
    expect(mixedInputDefaultsFor(platform)).toEqual({ ...defaultMixedInput, emoji: true });
    expect(settingsInputPreferences(undefined, platform).mixedInput.emoji).toBe(true);
  }
  for (const platform of ["linux", "android", "ios", "harmony", undefined, null]) {
    expect(mixedInputDefaultsFor(platform)).toBe(defaultMixedInput);
  }
  // 文档里有值时照文档，不看平台。
  const stored = { ...defaultMixedInput, emoji: false };
  expect(settingsInputPreferences({ mixed_input: stored }, "macos").mixedInput).toBe(stored);
});

test("读取旧文档时符号开、日文关，不补写可选键", () => {
  const preferences = Object.freeze({
    english: true,
    minimum_prefix: 8,
    emoji: true,
    kaomoji: false,
  });
  expect(mixedInputControlState(preferences)).toEqual({
    symbolsChecked: true,
    japaneseChecked: false,
    quickPhraseChecked: true,
    quickPhraseFrequencyChecked: true,
    minimumPrefixDisabled: false,
  });
  expect(Object.keys(preferences)).toEqual(["english", "minimum_prefix", "emoji", "kaomoji"]);
});

test("显式关闭符号和开启日文优先于缺省值", () => {
  expect(mixedInputControlState({ ...defaultMixedInput, symbols: false, japanese: true })).toEqual({
    symbolsChecked: false,
    japaneseChecked: true,
    quickPhraseChecked: true,
    quickPhraseFrequencyChecked: true,
    minimumPrefixDisabled: false,
  });
});

test("关闭英文仅置灰触发字符数，保留数值与其他混输选择", () => {
  const preferences = Object.freeze({
    english: false,
    minimum_prefix: 8,
    emoji: true,
    kaomoji: true,
    symbols: false,
    japanese: true,
  });
  expect(mixedInputControlState(preferences)).toEqual({
    symbolsChecked: false,
    japaneseChecked: true,
    quickPhraseChecked: true,
    quickPhraseFrequencyChecked: true,
    minimumPrefixDisabled: true,
  });
  expect(settingsInputPreferences({ mixed_input: preferences }).mixedInput).toBe(preferences);
  expect(preferences.minimum_prefix).toBe(8);
});
