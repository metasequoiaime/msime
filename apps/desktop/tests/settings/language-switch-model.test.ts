import { expect, test } from "vitest";
import { languageSwitchChoice, languageSwitchPatch } from "@msime/ui";
import { defaultKeybindings } from "../../../../packages/ui/src/settings/keybinding-defaults";

const disabled = Object.freeze({
  ...defaultKeybindings,
  switch_language_shift: false,
  switch_language_ctrl: false,
  switch_language_ctrl_space: false,
  switch_language_ctrl_alt_space: false,
});

test("缺少 Ctrl+Space 时只有 Linux 默认启用，读取不写回旧偏好", () => {
  const preferences = Object.freeze({ ...disabled, switch_language_ctrl_space: undefined });
  expect(languageSwitchChoice(preferences, true)).toBe("ctrl_space");
  expect(languageSwitchChoice(preferences, false)).toBe("none");
  expect(preferences.switch_language_ctrl_space).toBeUndefined();
  expect(languageSwitchChoice(disabled, true)).toBe("none");
});

test("多键同时启用时按 Shift、Ctrl、Linux Ctrl+Space、Ctrl+Alt+Space 排序", () => {
  const all = Object.freeze({
    ...disabled,
    switch_language_shift: true,
    switch_language_ctrl: true,
    switch_language_ctrl_space: true,
    switch_language_ctrl_alt_space: true,
  });
  for (const linux of [false, true]) {
    expect(languageSwitchChoice(all, linux)).toBe("shift");
    expect(languageSwitchChoice({ ...all, switch_language_shift: false }, linux)).toBe("ctrl");
  }
  const remaining = { ...all, switch_language_shift: false, switch_language_ctrl: false };
  expect(languageSwitchChoice(remaining, true)).toBe("ctrl_space");
  expect(languageSwitchChoice(remaining, false)).toBe("ctrl_alt_space");
  expect(all.switch_language_shift).toBe(true);
});

test("Linux 选择无时一次关闭四键，不改其他快捷键", () => {
  const patch = languageSwitchPatch("none", true);
  expect(patch).toEqual({
    switch_language_shift: false,
    switch_language_ctrl: false,
    switch_language_ctrl_space: false,
    switch_language_ctrl_alt_space: false,
  });
  expect({ ...defaultKeybindings, ...patch }).toMatchObject({
    switch_language_caps_lock: false,
    toggle_japanese_ctrl_shift_j: false,
    toggle_character_set_ctrl_shift_f: true,
    toggle_fullwidth_option_shift_h: true,
  });
});

test("非 Linux 的互斥补丁保留文档中已有的 Ctrl+Space 字段", () => {
  const patch = languageSwitchPatch("ctrl", false);
  expect(patch).toEqual({
    switch_language_shift: false,
    switch_language_ctrl: true,
    switch_language_ctrl_alt_space: false,
  });
  expect({ ...defaultKeybindings, ...patch }.switch_language_ctrl_space).toBe(true);
  expect(languageSwitchChoice({ ...defaultKeybindings, ...patch }, false)).toBe("ctrl");
});
