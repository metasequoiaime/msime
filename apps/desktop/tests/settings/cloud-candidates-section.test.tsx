// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CloudCandidatesSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("cloud candidates switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<CloudCandidatesSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("云候选"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("cloud candidates switch uses the enabled default", () => {
  render(<CloudCandidatesSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("云候选") as HTMLInputElement).checked).toBe(true);
});
