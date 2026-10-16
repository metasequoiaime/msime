import { expect, test } from "vitest";
import {
  enabledPagingShortcuts,
  navigationShortcutItems,
  wordCharacterKeyOptions,
} from "@msime/ui";
import { defaultNavigation } from "../../../../packages/ui/src/settings/candidate-paging-preferences";

test("翻页选项读取缺省滚轮为关，Harmony 隐藏它但不改已有偏好", () => {
  const legacy = Object.freeze({ ...defaultNavigation });
  expect(navigationShortcutItems(legacy).find(({ value }) => value === "mouse_wheel")).toEqual({
    value: "mouse_wheel",
    label: "鼠标滚轮（候选窗口支持时翻页）",
    checked: false,
  });
  expect(legacy).not.toHaveProperty("mouse_wheel");

  const saved = Object.freeze({ ...legacy, mouse_wheel: true, brackets: true });
  const desktop = navigationShortcutItems(saved);
  const phone = navigationShortcutItems(saved, true);
  expect(desktop.find(({ value }) => value === "mouse_wheel")?.checked).toBe(true);
  expect(phone.some(({ value }) => value === "mouse_wheel")).toBe(false);
  expect(phone.find(({ value }) => value === "brackets")?.checked).toBe(true);
  // 左右键移动候选同样是后加的可选键：桌面和手机都列出，文档里没有它时按关闭显示，桌面用按键对、手机用方向符号。
  expect(desktop.find(({ value }) => value === "left_right")).toEqual({
    value: "left_right",
    label: "左 / 右（移动候选项）",
    checked: false,
  });
  expect(phone.find(({ value }) => value === "left_right")).toEqual({
    value: "left_right",
    label: "← / →",
    checked: false,
  });
  expect(saved.mouse_wheel).toBe(true);
});

test("以词定字只禁用已被翻页占用的按键，重读时跟随新的翻页偏好", () => {
  const navigation = Object.freeze({ ...defaultNavigation, brackets: true, minus_equal: false });
  expect(wordCharacterKeyOptions(navigation)).toEqual([
    { value: "brackets", label: "[ / ]", disabled: true },
    { value: "minus_equal", label: "- / =", disabled: false },
  ]);
  expect(wordCharacterKeyOptions({ ...navigation, brackets: false, minus_equal: true })).toEqual([
    { value: "brackets", label: "[ / ]", disabled: false },
    { value: "minus_equal", label: "- / =", disabled: true },
  ]);
  expect(navigation.brackets).toBe(true);
});

test("翻页速查只列启用的翻页组合键，不混入滚轮或候选移动键", () => {
  const navigation = Object.freeze({
    minus_equal: false,
    comma_period: false,
    brackets: false,
    tab: false,
    page_up_down: false,
    mouse_wheel: true,
    arrows: true,
  });
  expect(enabledPagingShortcuts(navigation)).toEqual([]);
  expect(enabledPagingShortcuts({ ...navigation, brackets: true, page_up_down: true })).toEqual([
    ["brackets", "[ / ]"],
    ["page_up_down", "Page Up / Page Down"],
  ]);
  expect(navigation.brackets).toBe(false);
});
