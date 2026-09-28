// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DoubaoResourceIdSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards resource ID edits", () => {
  const onChange = vi.fn();
  render(<DoubaoResourceIdSection value="synthetic-resource" onChange={onChange} />);

  fireEvent.change(screen.getByLabelText("Doubao 资源 ID"), {
    target: { value: "updated-resource" },
  });

  expect(onChange).toHaveBeenCalledWith("updated-resource");
  expect(screen.getByText(/仅由 Doubao provider 使用/)).toBeTruthy();
});
