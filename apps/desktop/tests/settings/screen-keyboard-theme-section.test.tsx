// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ScreenKeyboardThemeSection, type SurfaceTheme } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("shows the mobile screen keyboard theme options", () => {
  const onChange = vi.fn<(value: SurfaceTheme) => void>();
  render(<ScreenKeyboardThemeSection mobile value="follow" onChange={onChange} />);

  expect(screen.getByText("覆盖颜色模式；当前屏幕键盘支持此设置")).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "屏幕键盘主题" }), {
    target: { value: "dark" },
  });
  expect(onChange).toHaveBeenCalledWith("dark");
});

test("uses the desktop description", () => {
  render(<ScreenKeyboardThemeSection mobile={false} value="light" onChange={vi.fn()} />);
  expect(screen.getByText("覆盖颜色模式；桌面屏幕键盘支持此设置")).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "屏幕键盘主题" }) as HTMLSelectElement).value).toBe(
    "light",
  );
});
