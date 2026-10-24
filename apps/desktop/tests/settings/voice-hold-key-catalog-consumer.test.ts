import { expect, test, vi } from "vitest";
import {
  holdChoice,
  holdKeyPatch,
  hotkeyOptions,
  selectedHoldKey,
} from "../../../../packages/ui/src/settings/voice-hotkeys";

vi.mock("../../../../packages/ui/src/settings/voice-hold-key-catalog", () => ({
  VOICE_HOLD_KEYS: ["hotkey_ctrl_win", "hotkey_ralt", "hotkey_rctrl_ralt"],
  VOICE_HOLD_PRESETS: [
    {
      key: "hotkey_ctrl_win",
      fallback: true,
      macos_label: "合成 Mac 首项",
      desktop_label: "合成桌面首项",
      other_label: "合成其他首项",
      macos_action: "合成 Mac 操作",
      desktop_action: "合成桌面操作",
      other_action: "合成其他操作",
    },
    {
      key: "hotkey_ralt",
      fallback: false,
      macos_label: "合成 Mac 次项",
      desktop_label: "合成桌面次项",
      other_label: "合成其他次项",
      macos_action: "合成 Mac 次操作",
      desktop_action: "合成桌面次操作",
      other_action: "合成其他次操作",
    },
    {
      key: "hotkey_rctrl_ralt",
      fallback: false,
      macos_label: "合成 Mac 末项",
      desktop_label: "合成桌面末项",
      other_label: "合成其他末项",
      macos_action: "合成 Mac 末操作",
      desktop_action: "合成桌面末操作",
      other_action: "合成其他末操作",
    },
  ],
}));

test("hold selection reads the shared catalog order and defaults", () => {
  expect(selectedHoldKey({})).toBe("hotkey_ctrl_win");
  expect(selectedHoldKey({ hotkey_ralt: true, hotkey_ctrl_win: true })).toBe("hotkey_ctrl_win");
  expect(selectedHoldKey({ hotkey_ctrl_win: false })).toBeNull();
  expect(Object.keys(holdKeyPatch(null))).toEqual([
    "hotkey_ctrl_win",
    "hotkey_ralt",
    "hotkey_rctrl_ralt",
  ]);
});

test("short choices and action labels project the shared platform names", () => {
  for (const [platform, label, action] of [
    ["macos", "合成 Mac 首项", "合成 Mac 操作"],
    ["windows", "合成桌面首项", "合成桌面操作"],
    ["other", "合成其他首项", "合成其他操作"],
  ] as const) {
    expect(holdChoice(platform).labels.hotkey_ctrl_win).toBe(label);
    expect(hotkeyOptions(platform).find(([key]) => key === "hotkey_ctrl_win")?.[1]).toBe(action);
  }
});
