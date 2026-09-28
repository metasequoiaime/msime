// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TelemetrySection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("telemetry switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<TelemetrySection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("匿名使用统计"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("telemetry switch is disabled by default", () => {
  render(<TelemetrySection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("匿名使用统计") as HTMLInputElement).checked).toBe(false);
});
