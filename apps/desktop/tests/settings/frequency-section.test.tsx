// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FrequencySection, type FrequencyPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: FrequencyPreferences = {
  mode: "promote",
  trigger_count: 1,
  linear_step: 1,
};

test("frequency section updates each preference independently", () => {
  const onChange = vi.fn();
  render(<FrequencySection preferences={preferences} onChange={onChange} />);

  fireEvent.change(screen.getByRole("combobox", { name: "调频方式" }), {
    target: { value: "linear" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, mode: "linear" });
  fireEvent.change(screen.getByRole("combobox", { name: "触发频次(第几次上屏触发)" }), {
    target: { value: "3" },
  });
  expect(onChange).toHaveBeenLastCalledWith({ ...preferences, trigger_count: 3 });
});

test("frequency section keeps persisted values above the standard option range", () => {
  render(
    <FrequencySection
      preferences={{ mode: "linear", trigger_count: 10, linear_step: 7 }}
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("combobox", { name: "触发频次(第几次上屏触发)" }).textContent).toContain(
    "10",
  );
  expect(screen.getByRole("combobox", { name: "线性调频步长" }).textContent).toContain("7");
});

test("each frequency row appears only where it has an effect", () => {
  const { rerender } = render(
    <FrequencySection preferences={{ ...preferences, mode: "linear" }} onChange={vi.fn()} />,
  );
  expect(screen.getByRole("combobox", { name: "线性调频步长" })).toBeTruthy();

  // 步长只有线性调频读。
  rerender(<FrequencySection preferences={{ ...preferences, mode: "halve" }} onChange={vi.fn()} />);
  expect(screen.getByRole("combobox", { name: "触发频次(第几次上屏触发)" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "线性调频步长" })).toBeNull();

  // 调频关闭时触发频次也不起作用。
  rerender(
    <FrequencySection preferences={{ ...preferences, mode: "disabled" }} onChange={vi.fn()} />,
  );
  expect(screen.getByRole("combobox", { name: "调频方式" })).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "触发频次(第几次上屏触发)" })).toBeNull();

  // 不学习选词习惯时整组都不画，存着的值不动。
  rerender(
    <FrequencySection
      preferences={{ ...preferences, mode: "linear" }}
      learning={false}
      onChange={vi.fn()}
    />,
  );
  expect(screen.queryByText("拼音方案调频")).toBeNull();
  expect(screen.queryByRole("combobox", { name: "调频方式" })).toBeNull();
});
