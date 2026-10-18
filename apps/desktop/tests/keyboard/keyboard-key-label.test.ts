import { expect, test } from "vitest";
import { isKeyboardLetter, keyboardKeyLabel, type KeyboardKey } from "@msime/ui";

const key = (label: string, shifted?: string): KeyboardKey => ({
  label,
  shifted,
  virtualKey: 0x41,
});

test.each<[string, boolean]>([
  ["a", true],
  ["Z", true],
  ["1", false],
  [";", false],
  ["é", false],
  ["ß", false],
  ["中", false],
  ["Enter", false],
  ["", false],
])("标签 %j 的 ASCII 字母分类为 %s", (label, expected) => {
  expect(isKeyboardLetter(key(label))).toBe(expected);
});

test.each<[KeyboardKey, boolean, string]>([
  [key("a"), false, "a"],
  [key("a"), true, "A"],
  [key("Z"), true, "Z"],
  [key("1", "!"), false, "1"],
  [key("1", "!"), true, "!"],
  [key(";", ":"), true, ":"],
  [key("a", "自定义"), true, "自定义"],
  [key("a", ""), true, "A"],
  [key("é"), true, "é"],
  [key("ß"), true, "ß"],
  [key("Enter"), true, "Enter"],
  [key("具名键", "显式标签"), true, "显式标签"],
])("按键 %j 在 shifted=%s 时使用标签 %j", (item, shifted, expected) => {
  expect(keyboardKeyLabel(item, shifted)).toBe(expected);
});
