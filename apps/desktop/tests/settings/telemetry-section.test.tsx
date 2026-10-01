// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TelemetrySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("usage reporting switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<TelemetrySection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("匿名使用统计"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("usage reporting is on when the preference is absent", () => {
  render(<TelemetrySection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("匿名使用统计") as HTMLInputElement).checked).toBe(true);
});

test("usage reporting shows off once the user turned it off", () => {
  render(<TelemetrySection value={false} onChange={vi.fn()} />);

  expect((screen.getByLabelText("匿名使用统计") as HTMLInputElement).checked).toBe(false);
});
