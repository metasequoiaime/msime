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
      "云剪贴板和云词库需要当前输入法进程提供输入会话；请从输入法菜单中的「云剪贴板…」打开云剪贴板。",
    ),
  ).toBeTruthy();
});
