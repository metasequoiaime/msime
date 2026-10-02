// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LegacyRadioGroup } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a shared legacy radio group with disabled options and dividers", () => {
  const onChange = vi.fn();
  const { container } = render(
    <LegacyRadioGroup
      title="输入方案"
      titleId="input-scheme-title"
      name="input-scheme"
      options={[
        { value: "quanpin", label: "全拼" },
        { value: "shuangpin", label: "双拼", disabled: true },
        { value: "wubi", label: "五笔" },
      ]}
      value="quanpin"
      description="选择输入方案"
      showDividers
      onChange={onChange}
    />,
  );

  expect(screen.getByRole("group", { name: "输入方案" })).toBeTruthy();
  expect((screen.getByRole("radio", { name: "全拼" }) as HTMLInputElement).checked).toBe(true);
  expect((screen.getByRole("radio", { name: "双拼" }) as HTMLInputElement).disabled).toBe(true);
  expect(container.querySelectorAll(".input-option-divider")).toHaveLength(2);
  expect(screen.getByText("选择输入方案")).toBeTruthy();

  fireEvent.click(screen.getByRole("radio", { name: "五笔" }));
  expect(onChange).toHaveBeenCalledWith("wubi");
});
