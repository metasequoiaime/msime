// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateFollowCursorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate follow-cursor switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<CandidateFollowCursorSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("候选窗口跟随光标"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("candidate follow-cursor switch uses the enabled default", () => {
  render(<CandidateFollowCursorSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("候选窗口跟随光标") as HTMLInputElement).checked).toBe(true);
});
