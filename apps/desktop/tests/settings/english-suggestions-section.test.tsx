// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { EnglishSuggestionsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("English suggestions switch reports the changed value", () => {
  const onChange = vi.fn();
  render(<EnglishSuggestionsSection value={true} onChange={onChange} />);

  fireEvent.click(screen.getByLabelText("英文建议"));
  expect(onChange).toHaveBeenCalledWith(false);
});

test("English suggestions switch uses the enabled default", () => {
  render(<EnglishSuggestionsSection value={undefined} onChange={vi.fn()} />);

  expect((screen.getByLabelText("英文建议") as HTMLInputElement).checked).toBe(true);
});
