// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TouchKeyboardSchemesSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const options = [
  ["quanpin", "全拼 26 键"],
  ["nine_key", "全拼 9 键"],
  ["wubi", "86 五笔"],
] as const;

test("selects a visible keyboard scheme and toggles another", () => {
  const onSelect = vi.fn();
  const onToggle = vi.fn();
  render(
    <TouchKeyboardSchemesSection
      options={options}
      enabled={["quanpin", "nine_key"]}
      selected="quanpin"
      onSelect={onSelect}
      onToggle={onToggle}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "设为当前输入方案 全拼 9 键" }));
  expect(onSelect).toHaveBeenCalledWith("nine_key");
  fireEvent.click(screen.getByRole("checkbox", { name: "显示输入方案 86 五笔" }));
  expect(onToggle).toHaveBeenCalledWith("wubi", true);
});

test("prevents disabling the last enabled scheme", () => {
  render(
    <TouchKeyboardSchemesSection
      options={options}
      enabled={["quanpin"]}
      selected="quanpin"
      onSelect={vi.fn()}
      onToggle={vi.fn()}
    />,
  );

  expect(
    (screen.getByRole("checkbox", { name: "显示输入方案 全拼 26 键" }) as HTMLInputElement)
      .disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "设为当前输入方案 全拼 9 键" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
});
