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
