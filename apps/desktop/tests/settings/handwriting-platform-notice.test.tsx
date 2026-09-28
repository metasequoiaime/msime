// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HandwritingPlatformNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test.each([
  ["ios", "手写输入", "首次在键盘中使用手写时下载中文模型"],
  ["harmony", "手写输入", "手写使用系统的文字识别能力"],
  ["android", "Android 手写输入", "首次在 Android 键盘中切换到手写时"],
] as const)("renders the %s handwriting notice", (platform, title, text) => {
  render(<HandwritingPlatformNotice platform={platform} />);

  expect(screen.getByText(title)).toBeTruthy();
  expect(screen.getByText(new RegExp(text))).toBeTruthy();
});

test("opens the SDK privacy link when the host provides one", () => {
  const onOpenExternalUrl = vi.fn();
  render(<HandwritingPlatformNotice platform="android" onOpenExternalUrl={onOpenExternalUrl} />);

  fireEvent.click(screen.getByRole("button", { name: "手写 SDK 隐私说明" }));
  expect(onOpenExternalUrl).toHaveBeenCalledWith("https://developers.google.com/ml-kit/terms");
});
