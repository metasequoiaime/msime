import { expect, test } from "vitest";
import { handwritingPrivacyText } from "@msime/ui";

test.each([
  ["ios", "首次在键盘中使用手写时下载中文模型"],
  ["harmony", "手写使用系统的文字识别能力"],
  ["android", "首次在 Android 键盘中切换到手写时"],
] as const)("describes the %s handwriting privacy terms", (platform, text) => {
  expect(handwritingPrivacyText(platform)).toContain(text);
});
