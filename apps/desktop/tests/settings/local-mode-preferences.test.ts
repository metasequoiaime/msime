import { expect, test } from "vitest";
import {
  localModeControlState,
  type LocalModeKey,
  type LocalModePreferences,
} from "../../../../packages/ui/src/settings/local-mode-preferences";

const preferences: LocalModePreferences = Object.freeze({
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
});

test("拆字缺省取决于宿主是否显示总开关，计算不写入可选字段", () => {
  expect(localModeControlState(preferences, "chaizi", true)).toEqual({
    checked: false,
    disabled: false,
  });
  expect(localModeControlState(preferences, "chaizi", false)).toEqual({
    checked: true,
    disabled: false,
  });
  expect(preferences).not.toHaveProperty("chaizi");
  expect(preferences).not.toHaveProperty("shift_letter_entry");
});

test.each([false, true])("拆字显式值 %s 优先于两种宿主的缺省", (checked) => {
  const previous = Object.freeze({ ...preferences, chaizi: checked });
  for (const masterVisible of [false, true]) {
    expect(localModeControlState(previous, "chaizi", masterVisible)).toEqual({
      checked,
      disabled: false,
    });
  }
});

test("总开关缺省开启，后加的 V、/、@ 开关缺省关闭", () => {
  expect(localModeControlState(preferences, "unicode", true)).toEqual({
    checked: true,
    disabled: false,
  });
  for (const key of ["expression", "command", "mention"] as const) {
    expect(localModeControlState(preferences, key, true)).toEqual({
      checked: false,
      disabled: false,
    });
  }
});

test("关闭总开关仅禁用字母入口模式且保留每项原勾选", () => {
  const previous = Object.freeze({
    ...preferences,
    shift_letter_entry: false,
    expression: true,
    chaizi: false,
  });
  const keys = [
    "unicode",
    "date_time",
    "quick_phrase",
    "emoji",
    "kaomoji",
    "super_jianpin",
    "temporary_english",
    "temporary_japanese",
    "expression",
    "chaizi",
  ] as const satisfies readonly LocalModeKey[];
  for (const key of keys) {
    expect(localModeControlState(previous, key, true)).toEqual({
      checked: previous[key],
      disabled: true,
    });
  }
  expect(previous.unicode).toBe(true);
  expect(previous.chaizi).toBe(false);
});

test("不显示总开关的宿主忽略文档里关闭的总开关", () => {
  const previous = Object.freeze({ ...preferences, shift_letter_entry: false });
  expect(localModeControlState(previous, "unicode", false)).toEqual({
    checked: true,
    disabled: false,
  });
  expect(localModeControlState(previous, "chaizi", false)).toEqual({
    checked: true,
    disabled: false,
  });
});

test("/ 和 @ 不受总开关约束，地名在名字模式开启时仍可改", () => {
  const previous = Object.freeze({
    ...preferences,
    shift_letter_entry: false,
    command: true,
    mention: true,
    mention_places: true,
  });
  for (const key of ["command", "mention", "mention_places"] as const) {
    expect(localModeControlState(previous, key, true)).toEqual({ checked: true, disabled: false });
  }
});

test.each([undefined, false])("名字模式为 %s 时地名保留勾选但不可改", (mention) => {
  const previous = Object.freeze({ ...preferences, mention, mention_places: true });
  for (const masterVisible of [false, true]) {
    expect(localModeControlState(previous, "mention_places", masterVisible)).toEqual({
      checked: true,
      disabled: true,
    });
  }
});
