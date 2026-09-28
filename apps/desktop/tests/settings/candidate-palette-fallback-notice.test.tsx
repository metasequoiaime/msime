// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CandidatePaletteFallbackNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains when mobile candidate colors follow the keyboard skin", () => {
  render(<CandidatePaletteFallbackNotice />);

  expect(
    screen.getByText(
      "候选栏正在使用键盘皮肤的颜色，下面的候选颜色要在「皮肤」页打开「使用桌面候选皮肤」后才生效。",
    ),
  ).toBeTruthy();
});
