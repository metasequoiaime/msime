// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AiCandidateLimitSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("clamps candidate count to the supported range", () => {
  const onChange = vi.fn();
  render(<AiCandidateLimitSection value={3} onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("AI 候选数量"), { target: { value: "99" } });
  expect(onChange).toHaveBeenCalledWith(10);
  fireEvent.change(screen.getByLabelText("AI 候选数量"), { target: { value: "0" } });
  expect(onChange).toHaveBeenCalledWith(3);
  expect(screen.getByText("候选数量")).toBeTruthy();
});
