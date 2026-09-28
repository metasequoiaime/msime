// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { HelpSettingsPage } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const common = {
  busy: false,
  hidden: false,
  mobile: false,
  ios: false,
  android: false,
  macos: false,
  platformHelpIntro: "平台简介",
  platformQuickStart: "快速开始",
  platformNetworkDescription: "网络说明",
};

test("renders macOS guide cards and opens documentation", () => {
  const onOpenDocumentation = vi.fn();
  render(<HelpSettingsPage {...common} macos onOpenDocumentation={onOpenDocumentation} />);

  expect(screen.getByRole("group", { name: "开始输入" })).toBeTruthy();
  expect(screen.getByText("候选词释义")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开官网" }));
  expect(onOpenDocumentation).toHaveBeenCalledOnce();
});

test("renders mobile help and opens system keyboard settings", () => {
  const onOpenSystemKeyboardSettings = vi.fn();
  render(
    <HelpSettingsPage
      {...common}
      mobile
      ios
      onOpenSystemKeyboardSettings={onOpenSystemKeyboardSettings}
    />,
  );

  expect(screen.getByText("平台简介")).toBeTruthy();
  expect(screen.getByText("允许完全访问")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "打开系统键盘设置" }));
  expect(onOpenSystemKeyboardSettings).toHaveBeenCalledOnce();
});

test("hides the page when requested", () => {
  const { container } = render(<HelpSettingsPage {...common} hidden />);
  expect(container.querySelector("fieldset")?.getAttribute("hidden")).not.toBeNull();
});
