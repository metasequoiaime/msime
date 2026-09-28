// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateLayoutSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate layout reports the selected orientation", () => {
  const onChange = vi.fn();
  render(<CandidateLayoutSection value="vertical" fixed={false} onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("候选项排列方式"), { target: { value: "horizontal" } });
  expect(onChange).toHaveBeenCalledWith("horizontal");
});

test("candidate layout hides when the host fixes the orientation", () => {
  render(<CandidateLayoutSection value="vertical" fixed onChange={vi.fn()} />);

  expect(screen.queryByLabelText("候选项排列方式")).toBeNull();
});
