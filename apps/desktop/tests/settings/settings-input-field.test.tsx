// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SettingsInputField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const mentionsView = Object.values(
  import.meta.glob<string>("../../../../packages/ui/src/settings/plugin-mentions-view.tsx", {
    eager: true,
    query: "?raw",
    import: "default",
  }),
)[0];

test("shared settings input field renders a stacked label and forwards native edits", () => {
  const onChange = vi.fn();
  render(
    <SettingsInputField
      label="合成字段"
      ariaLabel="合成输入"
      value="初始值"
      maxLength={12}
      placeholder="占位"
      onChange={onChange}
    />,
  );

  const input = screen.getByLabelText("合成输入") as HTMLInputElement;
  expect(screen.getByText("合成字段")).toBeTruthy();
  expect(input.className).toContain("rounded-md");
  expect(input.maxLength).toBe(12);
  expect(input.placeholder).toBe("占位");

  fireEvent.change(input, { target: { value: "更新值" } });
  expect(onChange).toHaveBeenCalledWith("更新值");
});

test("plugin mentions view reuses the shared stacked settings input", () => {
  expect(mentionsView).toContain('import { SettingsInputField } from "./settings-input-field";');
  expect(mentionsView).toContain("<SettingsInputField");
  expect(mentionsView).not.toContain("className={settings.fieldInput}");
});
