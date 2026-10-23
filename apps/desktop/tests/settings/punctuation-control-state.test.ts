import { expect, test } from "vitest";
import {
  punctuationControlState,
  type PunctuationPreferences,
} from "../../../../packages/ui/src/settings/punctuation-preferences";

const defaults = {
  chinese_punctuation: true,
  character_width: "halfwidth",
  smart_punctuation: true,
  smart_punctuation_repeat: true,
  smart_punctuation_space_convert: false,
  smart_punctuation_direct_digit: true,
  smart_punctuation_direct_letter: true,
  paired_punctuation: true,
  punctuation_lock: "follow",
  caps_lock_ascii_punctuation: false,
  quote_style: "curly",
  bracket_style: "solid",
  enumeration_comma_backslash: true,
  enumeration_comma_slash: false,
};

test("旧偏好缺少可选标点键时沿用现有控件缺省值", () => {
  const preferences = Object.freeze({ chinese_punctuation: true });
  expect(punctuationControlState(preferences)).toEqual(defaults);
  expect(preferences).toEqual({ chinese_punctuation: true });
});

test("显式关闭成对标点和中文标点时不套用缺省值", () => {
  expect(
    punctuationControlState({ chinese_punctuation: false, paired_punctuation: false }),
  ).toEqual({ ...defaults, chinese_punctuation: false, paired_punctuation: false });
});

test("保留已存的全部标点设置且不修改原偏好", () => {
  const preferences: Required<PunctuationPreferences> = Object.freeze({
    chinese_punctuation: false,
    character_width: "fullwidth",
    smart_punctuation: true,
    smart_punctuation_repeat: true,
    smart_punctuation_space_convert: true,
    smart_punctuation_direct_digit: true,
    smart_punctuation_direct_letter: true,
    paired_punctuation: false,
    punctuation_lock: "english",
    caps_lock_ascii_punctuation: true,
    quote_style: "corner_when_traditional",
    bracket_style: "hollow",
    enumeration_comma_backslash: true,
    enumeration_comma_slash: true,
  });
  expect(punctuationControlState(preferences)).toEqual(preferences);
  expect(punctuationControlState(preferences)).not.toBe(preferences);
});

test("中文固定标点和直角引号不受其他缺省字段影响", () => {
  expect(
    punctuationControlState({
      chinese_punctuation: true,
      punctuation_lock: "chinese",
      quote_style: "corner",
    }),
  ).toEqual({ ...defaults, punctuation_lock: "chinese", quote_style: "corner" });
});

test("顿号两个键都关时按「\\ 开」显示，与读出归一一致", () => {
  expect(
    punctuationControlState({
      chinese_punctuation: true,
      enumeration_comma_backslash: false,
      enumeration_comma_slash: false,
    }),
  ).toEqual(defaults);
  expect(
    punctuationControlState({
      chinese_punctuation: true,
      enumeration_comma_backslash: false,
      enumeration_comma_slash: true,
    }).enumeration_comma_backslash,
  ).toBe(false);
});
