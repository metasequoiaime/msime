// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidatePageSizeSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate page size reports the selected preset", () => {
  const onChange = vi.fn();
  render(<CandidatePageSizeSection value={5} fixed={false} onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("每页候选项数量"), { target: { value: "7" } });
  expect(onChange).toHaveBeenCalledWith(7);
});

test("candidate page size hides when the host fixes the page", () => {
  render(<CandidatePageSizeSection value={5} fixed onChange={vi.fn()} />);

  expect(screen.queryByLabelText("每页候选项数量")).toBeNull();
});
