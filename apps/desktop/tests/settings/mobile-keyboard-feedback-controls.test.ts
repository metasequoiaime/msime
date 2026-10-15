import { expect, test } from "vitest";
import {
  mobileKeyboardFeedbackControlState,
  type MobileKeyboardFeedback,
} from "../../../../packages/ui/src/settings/mobile-keyboard-feedback";

const settings: MobileKeyboardFeedback = {
  soundEnabled: true,
  hapticsEnabled: true,
  hapticStrength: "strong",
};

test("旧宿主缺省支持振动，英文建议缺键时开启", () => {
  expect(mobileKeyboardFeedbackControlState(settings)).toEqual({
    hapticsVisible: true,
    hapticStrengthVisible: true,
    englishSuggestionsChecked: true,
  });
});

test("设备不支持振动时隐藏控件，不清除已存选择", () => {
  const value = Object.freeze({ ...settings, hapticsAvailable: false });
  expect(mobileKeyboardFeedbackControlState(value)).toEqual({
    hapticsVisible: false,
    hapticStrengthVisible: false,
    englishSuggestionsChecked: true,
  });
  expect(value.hapticsEnabled).toBe(true);
  expect(value.hapticStrength).toBe("strong");
});

test("关闭振动只隐藏强度行，英文建议显式关闭优先", () => {
  const value = Object.freeze({ ...settings, hapticsEnabled: false, englishSuggestions: false });
  expect(mobileKeyboardFeedbackControlState(value)).toEqual({
    hapticsVisible: true,
    hapticStrengthVisible: false,
    englishSuggestionsChecked: false,
  });
  expect(value.hapticStrength).toBe("strong");
});
