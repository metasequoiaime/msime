// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateEnglishGlossSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate English gloss switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<CandidateEnglishGlossSection value={false} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("显示英文释义"));
  expect(onChange).toHaveBeenCalledWith(true);
});

test("candidate English gloss switch is disabled by default", () => {
  render(<CandidateEnglishGlossSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("显示英文释义") as HTMLInputElement).checked).toBe(false);
});
