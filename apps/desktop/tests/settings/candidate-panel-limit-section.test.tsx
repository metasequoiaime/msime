// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { CandidatePanelLimitSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders the host-specific candidate panel explanation", () => {
  render(<CandidatePanelLimitSection limit="fcitx_theme" />);
  expect(screen.getByText(/Fcitx5 正在使用你在 Fcitx5 配置中选择的经典界面主题/)).toBeTruthy();
});
