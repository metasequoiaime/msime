import { expect, test } from "vitest";
import {
  feedbackDiagnostics,
  feedbackMaxText,
  feedbackTypes,
} from "../../../../packages/ui/src/settings/feedback-contracts";

test("反馈契约保留服务类型和正文上限", () => {
  expect(feedbackMaxText).toBe(500);
  expect(feedbackTypes).toEqual([
    { value: "bug", label: "问题" },
    { value: "suggestion", label: "建议" },
    { value: "dictionary", label: "词库纠错" },
  ]);
});

test("反馈诊断只带有值的稳定字段", () => {
  expect(
    feedbackDiagnostics({
      appVersion: "0.52.0",
      edition: "desktop",
      host: { platform: "macos", os_version: "15.0" },
      preferences: {
        scheme: "pinyin",
        touch_keyboard_layout: "compact",
        global_theme: "light",
      },
      setup: { enabled: true, current: false },
    }),
  ).toEqual({
    os: "HarmonyOS 15.0",
    app_version: "0.52.0",
    edition: "desktop",
    scheme: "pinyin",
    keyboard_layout: "compact",
    skin: "light",
    ime_enabled: "true",
    ime_default: "false",
  });
  expect(feedbackDiagnostics({ appVersion: "0.52.0" })).toEqual({
    app_version: "0.52.0",
  });
});
