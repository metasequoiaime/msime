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
      "当前宿主以输入法菜单呈现工具栏，缩放和图标尺寸不适用；组件选择仍然生效，上方开关仍然生效。",
    ),
  ).toBeTruthy();
});
