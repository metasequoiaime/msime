// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LearningSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("learning switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<LearningSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("学习选词习惯"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("learning switch reflects the supplied value", () => {
  render(<LearningSection value={false} onChange={vi.fn()} />);

  expect((screen.getByLabelText("学习选词习惯") as HTMLInputElement).checked).toBe(false);
});
