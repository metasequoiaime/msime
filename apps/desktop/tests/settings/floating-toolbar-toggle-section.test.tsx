// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FloatingToolbarToggleSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences = {
  enabled: true,
  english_mode: true,
  fullwidth: true,
  punctuation: true,
  character_set: true,
  emoji: false,
  handwriting: false,
  screen_keyboard: false,
  voice: false,
  settings: true,
  scale_percent: 100 as const,
  font_size: 24 as const,
};

test("changes the floating toolbar visibility and renders its preview", () => {
  const onEnabledChange = vi.fn();
  render(
    <FloatingToolbarToggleSection
      preferences={preferences}
      skin="willow_green"
      theme="dark"
      onEnabledChange={onEnabledChange}
    />,
  );

  fireEvent.click(screen.getByRole("checkbox", { name: "在桌面显示悬浮工具栏" }));
  expect(onEnabledChange).toHaveBeenCalledWith(false);
  expect(screen.getByLabelText("悬浮工具栏预览")).toBeTruthy();
});
