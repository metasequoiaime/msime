// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { SettingsTextareaField } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("renders a labelled settings textarea and forwards changes", () => {
  const onChange = vi.fn();
  render(
    <SettingsTextareaField
      label="描述"
      ariaLabel="反馈描述"
      description="请说明遇到的问题"
      value="synthetic detail"
      rows={6}
      maxLength={4000}
      placeholder="输入描述"
      onChange={onChange}
    />,
  );

  const textarea = screen.getByRole("textbox", { name: "反馈描述" }) as HTMLTextAreaElement;
  expect(screen.getByText("描述")).toBeTruthy();
  expect(screen.getByText("请说明遇到的问题")).toBeTruthy();
  expect(textarea.value).toBe("synthetic detail");
  expect(textarea.rows).toBe(6);
  expect(textarea.maxLength).toBe(4000);
  expect(textarea.placeholder).toBe("输入描述");

  fireEvent.change(textarea, { target: { value: "updated detail" } });
  expect(onChange).toHaveBeenCalledWith("updated detail");
});

test("forwards disabled and custom textarea classes", () => {
  render(
    <SettingsTextareaField
      label="提示词"
      ariaLabel="测试提示词"
      value=""
      disabled
      className="custom-textarea"
      onChange={vi.fn()}
    />,
  );

  const textarea = screen.getByRole("textbox", { name: "测试提示词" }) as HTMLTextAreaElement;
  expect(textarea.disabled).toBe(true);
  expect(textarea.className).toBe("custom-textarea");
});
