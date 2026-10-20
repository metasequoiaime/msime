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

test("marks the mobile system settings action busy", () => {
  const onOpenSystemKeyboardSettings = vi.fn();
  render(
    <HelpSettingsPage
      {...common}
      busy
      mobile
      ios
      onOpenSystemKeyboardSettings={onOpenSystemKeyboardSettings}
    />,
  );

  const button = screen.getByRole("button", { name: "打开系统键盘设置" });
  expect(button.getAttribute("aria-busy")).toBe("true");
  expect((button as HTMLButtonElement).disabled).toBe(true);
});

test("hides the page when requested", () => {
  const { container } = render(<HelpSettingsPage {...common} hidden />);
  expect(container.querySelector("fieldset")?.getAttribute("hidden")).not.toBeNull();
});

test("Linux 也用术语卡片，键名和排查步骤是 Linux 的，并提供打开官网", () => {
  const onOpenDocumentation = vi.fn();
  render(<HelpSettingsPage {...common} linux onOpenDocumentation={onOpenDocumentation} />);

  const start = screen.getByRole("group", { name: "开始输入" });
  expect(start.textContent).toContain("Ctrl+Shift+F");
  expect(start.textContent).not.toContain("Option");
  const problems = screen.getByRole("group", { name: "遇到问题" });
  expect(problems.textContent).toContain("fcitx5-configtool");
  expect(problems.textContent).toContain("ibus restart");
  expect(problems.textContent).toContain("Ctrl+Shift+Alt+R");
  // 云候选、语音和 AI 走用户级服务、凭据存在本机文件里，这段联网说明留在 Linux 的帮助页上。
  expect(screen.getByText("网络说明")).toBeTruthy();
  // 散文版的「快速上手」不再出现，macOS 那一行「打开官网」也给 Linux。
  expect(screen.queryByText("快速开始")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "打开官网" }));
  expect(onOpenDocumentation).toHaveBeenCalledOnce();
});
