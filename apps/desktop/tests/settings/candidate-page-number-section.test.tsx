// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePageNumberSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("page numbers default on and toggle independently", () => {
  const onChange = vi.fn();
  const { rerender } = render(<CandidatePageNumberSection value={undefined} onChange={onChange} />);
  const toggle = screen.getByLabelText("显示页码") as HTMLInputElement;
  expect(toggle.checked).toBe(true);
  fireEvent.click(toggle);
  expect(onChange).toHaveBeenCalledWith(false);
  rerender(<CandidatePageNumberSection value={false} onChange={onChange} />);
  expect((screen.getByLabelText("显示页码") as HTMLInputElement).checked).toBe(false);
  fireEvent.click(screen.getByLabelText("显示页码"));
  expect(onChange).toHaveBeenCalledWith(true);
});
