// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePageIndicatorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("page number switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<CandidatePageIndicatorSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("显示页码"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("page number switch is on when the preference is absent", () => {
  render(<CandidatePageIndicatorSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("显示页码") as HTMLInputElement).checked).toBe(true);
});
