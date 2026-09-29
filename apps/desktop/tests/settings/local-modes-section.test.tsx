// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LocalModesSection, type LocalModePreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
};

test("local mode section updates only the selected mode", () => {
  const onChange = vi.fn();
  render(<LocalModesSection preferences={preferences} ios={false} onChange={onChange} />);

  fireEvent.click(screen.getByRole("switch", { name: /^Unicode/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, unicode: false });
});

test("iOS local mode descriptions use the touch keyboard entry path", () => {
  render(<LocalModesSection preferences={preferences} ios onChange={vi.fn()} />);

  expect(screen.getByText(/「更多 → 本地输入」里选「快捷短语」/)).toBeTruthy();
  expect(screen.queryByText(/Shift\+K/)).toBeNull();
});
