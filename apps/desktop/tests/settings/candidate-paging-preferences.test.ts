import { expect, test } from "vitest";
import {
  defaultNavigation,
  pagingShortcutChange,
  restorePagingChange,
  pagingPreferencesPatch,
  wordCharacterEnabledChange,
  type WordCharacterPreferences,
} from "../../../../packages/ui/src/settings/candidate-paging-preferences";

test.each(["brackets", "minus_equal"] as const)("打开 %s 翻页时让出同组以词定字按键", (key) => {
  const navigation = Object.freeze({ ...defaultNavigation, [key]: false });
  const wordCharacter = Object.freeze({ enabled: true, keys: key });
  const next = pagingShortcutChange(navigation, wordCharacter, key, true);
  expect(next.navigation).toEqual({ ...navigation, [key]: true });
  expect(next.navigation).not.toBe(navigation);
  expect(next.wordCharacter).toEqual({ enabled: false, keys: key });
  expect(wordCharacter.enabled).toBe(true);
  expect(navigation[key]).toBe(false);
});

test.each([
  ["tab", false],
  ["comma_period", true],
  ["brackets", false],
] as const)("修改 %s 为 %s 不修复已有无关冲突", (key, enabled) => {
  const navigation = Object.freeze({ ...defaultNavigation, brackets: true });
  const wordCharacter = Object.freeze({ enabled: true, keys: "brackets" as const });
  const next = pagingShortcutChange(navigation, wordCharacter, key, enabled);
  expect(next.wordCharacter).toBe(wordCharacter);
  expect(next.navigation).toEqual({ ...navigation, [key]: enabled });
  expect(next.navigation).not.toHaveProperty("mouse_wheel");
});

test("只有主动修改滚轮时才在翻页偏好中加入可选字段", () => {
  const wordCharacter = { enabled: false, keys: "brackets" as const };
  const next = pagingShortcutChange(defaultNavigation, wordCharacter, "mouse_wheel", true);
  expect(next.navigation.mouse_wheel).toBe(true);
  expect(defaultNavigation).not.toHaveProperty("mouse_wheel");
  expect(next.wordCharacter).toBe(wordCharacter);
});

test("已关闭的以词定字不会因同组翻页打开而换对象", () => {
  const wordCharacter = { enabled: false, keys: "brackets" as const };
  expect(
    pagingShortcutChange(defaultNavigation, wordCharacter, "brackets", true).wordCharacter,
  ).toBe(wordCharacter);
});

test("恢复默认只让出被默认翻页占用的以词定字按键", () => {
  const conflict = Object.freeze({ enabled: true, keys: "minus_equal" as const });
  const restored = restorePagingChange(conflict);
  expect(restored.navigation).toBe(defaultNavigation);
  expect(restored.wordCharacter).toEqual({ enabled: false, keys: "minus_equal" });
  expect(conflict.enabled).toBe(true);
  for (const wordCharacter of [
    { enabled: true, keys: "brackets" },
    { enabled: false, keys: "minus_equal" },
  ] satisfies WordCharacterPreferences[]) {
    expect(restorePagingChange(wordCharacter).wordCharacter).toBe(wordCharacter);
  }
});

test("以词定字开关只在开启时复制并关闭所占翻页键", () => {
  const wordCharacter = Object.freeze({ enabled: false, keys: "minus_equal" as const });
  const navigation = Object.freeze({ ...defaultNavigation });
  const enabled = wordCharacterEnabledChange(wordCharacter, navigation, true);
  expect(enabled.wordCharacter).toEqual({ enabled: true, keys: "minus_equal" });
  expect(enabled.navigation).toEqual({ ...navigation, minus_equal: false });
  const disabled = wordCharacterEnabledChange(enabled.wordCharacter, enabled.navigation, false);
  expect(disabled.navigation).toBe(enabled.navigation);
  expect(disabled.wordCharacter).toEqual(wordCharacter);
  expect(navigation.minus_equal).toBe(true);
});

test("翻页补丁按引用变化决定是否包含以词定字字段", () => {
  const wordCharacter = { enabled: true, keys: "brackets" as const };
  const unchanged = pagingPreferencesPatch(wordCharacter, {
    navigation: defaultNavigation,
    wordCharacter,
  });
  expect(unchanged).toEqual({ navigation: defaultNavigation });
  expect(unchanged).not.toHaveProperty("word_character");
  const copied = { ...wordCharacter };
  const changed = pagingPreferencesPatch(wordCharacter, {
    navigation: defaultNavigation,
    wordCharacter: copied,
  });
  expect(changed.word_character).toBe(copied);
  expect(changed.navigation).toBe(defaultNavigation);
});
