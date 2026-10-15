import { expect, test } from "vitest";
import {
  floatingToolbarButtonOptions,
  floatingToolbarFontSizes,
  floatingToolbarScales,
} from "@msime/ui";
import { defaultFloatingToolbar } from "../../../../packages/ui/src/settings/floating-toolbar-defaults";

test("悬浮工具栏按钮按显示顺序过滤宿主能力，保留必选模式切换", () => {
  const withoutOptional = floatingToolbarButtonOptions(defaultFloatingToolbar, {
    floating_toolbar_input_scheme: false,
    floating_toolbar_handwriting: false,
    floating_toolbar_voice: false,
  });
  expect(withoutOptional.map(({ value }) => value)).toEqual([
    "mode_switch",
    "english_mode",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "screen_keyboard",
    "settings",
  ]);
  expect(withoutOptional[0]).toMatchObject({ checked: true, disabled: true });

  const withAll = floatingToolbarButtonOptions(defaultFloatingToolbar, {
    floating_toolbar_input_scheme: true,
    floating_toolbar_handwriting: true,
    floating_toolbar_voice: true,
  });
  expect(withAll.map(({ value }) => value)).toEqual([
    "mode_switch",
    "english_mode",
    "input_scheme",
    "fullwidth",
    "punctuation",
    "character_set",
    "emoji",
    "handwriting",
    "screen_keyboard",
    "voice",
    "settings",
  ]);
  expect(withAll[0]).toMatchObject({ checked: true, disabled: true });
});

test("悬浮工具栏尺寸选项保持固定顺序", () => {
  expect(floatingToolbarScales).toEqual([75, 100, 125, 150]);
  expect(floatingToolbarFontSizes).toEqual([16, 18, 20, 22, 24, 26, 28]);
});
