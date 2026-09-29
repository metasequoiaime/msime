// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SkinPlatformNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test.each([
  [true, false, "选择候选栏和键盘使用的主题；明暗预览仅影响当前卡片，不修改设置。"],
  [false, true, "选择候选窗使用的主题；明暗预览仅影响当前卡片，不修改设置。"],
  [false, false, "选择候选窗和悬浮工具栏使用的主题；明暗预览仅影响当前卡片，不修改设置。"],
] as const)("renders the platform-specific skin explanation", (mobile, linux, text) => {
  render(<SkinPlatformNotice mobile={mobile} linux={linux} />);

  expect(screen.getByText(text)).toBeTruthy();
});
