import { expect, test } from "vitest";
import {
  VOICE_HOLD_KEYS,
  holdChoice,
  holdKeyPatch,
  hotkeyOptions,
  selectedHoldKey,
  type VoiceHoldKey,
  type VoiceHotkeyPlatform,
} from "../../../../packages/ui/src/settings/voice-hotkeys";

test("hold choices preserve missing values, explicit switches and priority for all 27 profiles", () => {
  const keys = ["hotkey_ralt", "hotkey_rctrl_ralt", "hotkey_ctrl_win"] as const;
  const states = [undefined, false, true] as const;
  expect(VOICE_HOLD_KEYS).toEqual(keys);
  for (const first of states) {
    for (const second of states) {
      for (const third of states) {
        const input = Object.freeze({
          hotkey_ralt: first,
          hotkey_rctrl_ralt: second,
          hotkey_ctrl_win: third,
        });
        const before = { ...input };
        const expected =
          (first ?? true)
            ? "hotkey_ralt"
            : (second ?? false)
              ? "hotkey_rctrl_ralt"
              : (third ?? false)
                ? "hotkey_ctrl_win"
                : null;
        expect(selectedHoldKey(input)).toBe(expected);
        expect(input).toEqual(before);
      }
    }
  }
});

test("choosing a hold key replaces exactly the three switches", () => {
  for (const key of [...VOICE_HOLD_KEYS, null]) {
    expect(holdKeyPatch(key)).toEqual({
      hotkey_ralt: key === "hotkey_ralt",
      hotkey_rctrl_ralt: key === "hotkey_rctrl_ralt",
      hotkey_ctrl_win: key === "hotkey_ctrl_win",
    });
  }
});

test("platform projections preserve their titles and both short and action labels", () => {
  const platforms: VoiceHotkeyPlatform[] = ["macos", "windows", "linux", "harmony", "other"];
  for (const platform of platforms) {
    const macos = platform === "macos";
    const other = platform === "other";
    const expected: Record<VoiceHoldKey, string> = {
      hotkey_ralt: macos ? "右 Option" : "右 Alt",
      hotkey_rctrl_ralt: macos ? "右 Control+右 Option" : other ? "Ctrl+右 Alt" : "右 Ctrl+右 Alt",
      hotkey_ctrl_win: macos ? "Control+Command" : "Ctrl+Win",
    };
    expect(holdChoice(platform)).toEqual({
      title: macos ? "按住录音" : other ? "切换语音" : "长按录音",
      labels: expected,
    });
    expect(hotkeyOptions(platform)).toEqual([
      ["hotkey_ctrl_f9", "Ctrl+F9 切换语音"],
      ...VOICE_HOLD_KEYS.map((key) => [
        key,
        other
          ? `${expected[key]} 切换语音`
          : `${macos ? "按住" : "长按"}${key === "hotkey_ctrl_win" ? " " : ""}${expected[key]} 录音`,
      ]),
      ["hotkey_hold_space_lock", macos || other ? "空格锁定语音" : "长按录音时按空格锁定"],
    ]);
  }
});
