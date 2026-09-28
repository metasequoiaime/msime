// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TraditionalChineseOutputSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("traditional output switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<TraditionalChineseOutputSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("简繁输入"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("traditional output switch is disabled by default", () => {
  render(<TraditionalChineseOutputSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("简繁输入") as HTMLInputElement).checked).toBe(false);
});
