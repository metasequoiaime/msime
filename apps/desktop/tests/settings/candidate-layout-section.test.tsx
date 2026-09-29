// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CandidateLayoutSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("candidate layout reports the selected orientation", () => {
  const onChange = vi.fn();
  render(<CandidateLayoutSection value="vertical" fixed={false} onChange={onChange} />);

  const layout = screen.getByRole("radiogroup", { name: "候选项排列方式" });
  expect((within(layout).getByRole("radio", { name: "纵向" }) as HTMLInputElement).checked).toBe(
    true,
  );
  fireEvent.click(within(layout).getByRole("radio", { name: "横向" }));
  expect(onChange).toHaveBeenCalledWith("horizontal");
});

test("candidate layout hides when the host fixes the orientation", () => {
  render(<CandidateLayoutSection value="vertical" fixed onChange={vi.fn()} />);

  expect(screen.queryByLabelText("候选项排列方式")).toBeNull();
});
