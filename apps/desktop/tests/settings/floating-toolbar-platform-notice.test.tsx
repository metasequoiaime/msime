// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { FloatingToolbarPlatformNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains that Linux presents the toolbar as an input method menu", () => {
  render(<FloatingToolbarPlatformNotice />);

  expect(
    screen.getByText(
      "Linux 不显示悬浮工具栏窗口，工具栏以输入法菜单里的「工具栏」子菜单呈现，缩放和图标尺寸不适用；下方的按钮选择和显示开关仍然生效。GNOME 桌面下的 IBus 只在输入源菜单列出输入模式和设置，没有这个子菜单。",
    ),
  ).toBeTruthy();
});

test("names only the display switch on a host without the button checks", () => {
  render(<FloatingToolbarPlatformNotice buttons={false} />);

  expect(
    screen.getByText(
      "Linux 不显示悬浮工具栏窗口，工具栏以输入法菜单里的「工具栏」子菜单呈现，缩放和图标尺寸不适用；下方的显示开关仍然生效。GNOME 桌面下的 IBus 只在输入源菜单列出输入模式和设置，没有这个子菜单。",
    ),
  ).toBeTruthy();
});

test("says the page has no effect under GNOME Shell, where IBus publishes no toolbar menu", () => {
  render(<FloatingToolbarPlatformNotice gnomeShell />);

  expect(
    screen.getByText(
      "Linux 不显示悬浮工具栏窗口。当前是 GNOME 桌面，IBus 在输入源菜单里只列出输入模式和设置，没有「工具栏」子菜单，这一页的设置在当前桌面不生效。",
    ),
  ).toBeTruthy();
  expect(screen.queryByText(/仍然生效/)).toBeNull();
});
