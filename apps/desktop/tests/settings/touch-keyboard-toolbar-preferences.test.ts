import { expect, test } from "vitest";
import {
  harmonyToolbarOptions,
  harmonyToolbarPreferences,
  touchToolbarPreferences,
  touchToolbarOptions,
  type TouchToolbarPreferences,
} from "../../../../packages/ui/src/settings/touch-keyboard-toolbar-preferences";

const defaults = {
  layout: true,
  emoji: true,
  skin: true,
  clipboard: false,
  ai: false,
  character_set: false,
  fullwidth: false,
  punctuation: false,
};

test("默认工具栏保留键盘已有按钮，并按固定顺序公开选项", () => {
  expect(touchToolbarPreferences()).toEqual(defaults);
  expect(touchToolbarOptions.map(([key]) => key)).toEqual([
    "layout",
    "emoji",
    "skin",
    "clipboard",
    "ai",
    "character_set",
    "fullwidth",
    "punctuation",
  ]);
});

test("缺键按默认值显示，显式 false 覆盖默认值", () => {
  const source: Partial<TouchToolbarPreferences> = Object.freeze({
    clipboard: true,
    skin: false,
  });
  expect(touchToolbarPreferences(source)).toEqual({
    ...defaults,
    clipboard: true,
    skin: false,
  });
  expect(source).toEqual({ clipboard: true, skin: false });
});

test("每次解析返回新对象，不把显示缺省值写回输入", () => {
  const source = Object.freeze({ emoji: false });
  const first = touchToolbarPreferences(source);
  const second = touchToolbarPreferences(source);
  expect(first).toEqual({ ...defaults, emoji: false });
  expect(second).not.toBe(first);
  expect(source).toEqual({ emoji: false });
});

test("鸿蒙工具栏缺省五个工具全在、不隐藏，缺键按缺省补齐且不改输入", () => {
  expect(harmonyToolbarPreferences()).toEqual({
    emoji: true,
    phrase: true,
    clipboard: true,
    skin: true,
    scheme: true,
    hidden: false,
  });
  expect(harmonyToolbarOptions.map(([key]) => key)).toEqual([
    "emoji",
    "phrase",
    "clipboard",
    "skin",
    "scheme",
  ]);
  const source = Object.freeze({ skin: false, hidden: true });
  expect(harmonyToolbarPreferences(source)).toEqual({
    emoji: true,
    phrase: true,
    clipboard: true,
    skin: false,
    scheme: true,
    hidden: true,
  });
  expect(source).toEqual({ skin: false, hidden: true });
});
