// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { FloatingToolbarPlatformNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains which toolbar controls are unavailable on menu hosts", () => {
  render(<FloatingToolbarPlatformNotice />);

  expect(
    screen.getByText(
      "当前宿主以输入法菜单呈现工具栏，缩放和图标尺寸不适用；上方的按钮选择和显示开关仍然生效。",
    ),
  ).toBeTruthy();
});

test("names only the display switch on a host without the button checks", () => {
  render(<FloatingToolbarPlatformNotice buttons={false} />);

  expect(
    screen.getByText(
      "当前宿主以输入法菜单呈现工具栏，缩放和图标尺寸不适用；上方的显示开关仍然生效。",
    ),
  ).toBeTruthy();
});
