// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SelectSettingField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled legacy select and forwards value changes", () => {
  const onChange = vi.fn();
  render(
    <SelectSettingField
      label="服务提供商"
      inputLabel="AI 服务提供商"
      description="选择服务商"
      value="synthetic-provider"
      onChange={onChange}
    >
      <option value="synthetic-provider">Synthetic</option>
      <option value="other-provider">Other</option>
    </SelectSettingField>,
  );

  const select = screen.getByLabelText("AI 服务提供商") as HTMLSelectElement;
  expect(screen.getByText("服务提供商")).toBeTruthy();
  expect(screen.getByText("选择服务商")).toBeTruthy();
  expect(select.value).toBe("synthetic-provider");
  expect(select.options).toHaveLength(2);

  fireEvent.change(select, { target: { value: "other-provider" } });
  expect(onChange).toHaveBeenCalledWith("other-provider");
});

test("passes native select state through", () => {
  render(
    <SelectSettingField
      label="地域"
      inputLabel="腾讯云地域"
      value="ap-test"
      disabled
      required
      onChange={vi.fn()}
    >
      <option value="ap-test">ap-test</option>
    </SelectSettingField>,
  );

  const select = screen.getByLabelText("腾讯云地域") as HTMLSelectElement;
  expect(select.disabled).toBe(true);
  expect(select.required).toBe(true);
});
