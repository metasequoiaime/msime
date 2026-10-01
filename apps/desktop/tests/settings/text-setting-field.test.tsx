// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TextSettingField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled legacy field and forwards text changes", () => {
  const onChange = vi.fn();
  render(
    <TextSettingField
      label="模型"
      inputLabel="AI 模型"
      description="由服务商选择模型"
      value="synthetic-model"
      placeholder="模型名称"
      onChange={onChange}
    />,
  );

  const input = screen.getByLabelText("AI 模型") as HTMLInputElement;
  expect(screen.getByText("模型")).toBeTruthy();
  expect(screen.getByText("由服务商选择模型")).toBeTruthy();
  expect(input.value).toBe("synthetic-model");
  expect(input.placeholder).toBe("模型名称");

  fireEvent.change(input, { target: { value: "updated-model" } });
  expect(onChange).toHaveBeenCalledWith("updated-model");
});

test("passes native input state through", () => {
  render(
    <TextSettingField
      label="地域"
      inputLabel="腾讯云地域"
      value="ap-test"
      disabled
      spellCheck={false}
      onChange={vi.fn()}
    />,
  );

  const input = screen.getByLabelText("腾讯云地域") as HTMLInputElement;
  expect(input.disabled).toBe(true);
  expect(input.getAttribute("spellcheck")).toBe("false");
});
