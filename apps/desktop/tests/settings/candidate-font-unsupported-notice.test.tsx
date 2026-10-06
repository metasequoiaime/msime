// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CandidateFontUnsupportedNotice } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("explains when the host cannot customize candidate fonts or sizes", () => {
  render(<CandidateFontUnsupportedNotice />);

  expect(screen.getByText("当前宿主的候选窗口不支持自定义字体或字号。")).toBeTruthy();
});
