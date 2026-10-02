// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CloudPanelSessionNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains how macOS opens the cloud clipboard", () => {
  render(<CloudPanelSessionNotice />);

  expect(
    screen.getByText(
      "云剪贴板需要当前输入法进程提供输入会话才能直接粘贴；请从输入法菜单中的「云剪贴板…」打开。",
    ),
  ).toBeTruthy();
});
