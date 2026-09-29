// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { ShortcutsIntroSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("describes shortcut scope for a mobile candidate bar", () => {
  render(<ShortcutsIntroSection mobile />);

  expect(
    screen.getByText(
      "输入法快捷键仅在对应输入状态或候选栏显示时生效。翻页方式可在“输入”中启用或关闭。",
    ),
  ).toBeTruthy();
});

test("describes shortcut scope for a desktop candidate window", () => {
  render(<ShortcutsIntroSection mobile={false} />);

  expect(
    screen.getByText(
      "输入法快捷键仅在对应输入状态或候选窗口显示时生效。翻页方式可在“输入”中启用或关闭。",
    ),
  ).toBeTruthy();
});
