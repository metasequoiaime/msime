// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputSchemeSelectorSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("shows the selected scheme and forwards radio changes", () => {
  const onChange = vi.fn();
  render(<InputSchemeSelectorSection value="quanpin" onChange={onChange} />);

  expect((screen.getByRole("radio", { name: "全拼" }) as HTMLInputElement).checked).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: "双拼" }));
  expect(onChange).toHaveBeenCalledWith("shuangpin");
  expect(screen.getByRole("group", { name: "输入方案" })).toBeTruthy();
});

test("grouped mode renders the selector in a platform row and honors hidden state", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <InputSchemeSelectorSection grouped value="quanpin" onChange={onChange} />,
  );

  expect(screen.getByRole("radiogroup")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "双拼" }));
  expect(onChange).toHaveBeenCalledWith("shuangpin");

  rerender(<InputSchemeSelectorSection grouped hidden value="quanpin" onChange={onChange} />);
  expect(screen.getByText("输入方案").closest("div")?.hasAttribute("hidden")).toBe(true);
});
