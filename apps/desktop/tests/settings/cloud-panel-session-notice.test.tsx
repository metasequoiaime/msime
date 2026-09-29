// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CloudPanelSessionNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains how macOS opens cloud panels", () => {
  render(<CloudPanelSessionNotice />);

  expect(
    screen.getByText(
      "云剪贴板和云词典需要当前输入法进程提供输入会话；请从输入法悬浮工具栏或输入法菜单打开对应面板。",
    ),
  ).toBeTruthy();
});
