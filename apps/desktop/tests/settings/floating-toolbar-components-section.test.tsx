// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FloatingToolbarComponentsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const values = {
  english_mode: true,
  fullwidth: true,
  punctuation: true,
  character_set: true,
  emoji: false,
  handwriting: false,
  screen_keyboard: false,
  voice: false,
  settings: true,
};

test("filters unavailable host components and forwards changes", () => {
  const onChange = vi.fn();
  render(
    <FloatingToolbarComponentsSection
      values={values}
      capabilities={{ floating_toolbar_handwriting: false, floating_toolbar_voice: true }}
      onChange={onChange}
    />,
  );

  expect(screen.queryByRole("checkbox", { name: "手写识别板" })).toBeNull();
  const voice = screen.getByRole("checkbox", { name: "语音输入" }) as HTMLInputElement;
  expect(voice.checked).toBe(false);
  fireEvent.click(voice);
  expect(onChange).toHaveBeenCalledWith("voice", true);
  expect(screen.getByText("始终显示")).toBeTruthy();
});

test("shows capability-gated options when host capabilities are unknown", () => {
  render(<FloatingToolbarComponentsSection values={values} onChange={vi.fn()} />);
  expect(screen.getByRole("checkbox", { name: "手写识别板" })).toBeTruthy();
  expect(screen.getByRole("checkbox", { name: "语音输入" })).toBeTruthy();
});
