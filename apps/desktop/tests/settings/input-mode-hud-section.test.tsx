// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeHudSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("input mode HUD switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<InputModeHudSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("中英文切换提示"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("shortcut placement uses the enabled default and its shortcut label", () => {
  render(<InputModeHudSection shortcut value={undefined} onChange={vi.fn()} />);

  const toggle = screen.getByLabelText("切换中英文时显示提示") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
});
