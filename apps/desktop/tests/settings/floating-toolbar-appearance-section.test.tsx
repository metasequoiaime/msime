// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { FloatingToolbarAppearanceSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("forwards toolbar scale and icon size selections", () => {
  const onScaleChange = vi.fn();
  const onFontSizeChange = vi.fn();
  render(
    <FloatingToolbarAppearanceSection
      scale={100}
      fontSize={24}
      onScaleChange={onScaleChange}
      onFontSizeChange={onFontSizeChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("工具栏缩放"), { target: { value: "150" } });
  fireEvent.change(screen.getByLabelText("图标尺寸"), { target: { value: "18" } });
  expect(onScaleChange).toHaveBeenCalledWith(150);
  expect(onFontSizeChange).toHaveBeenCalledWith(18);
  expect(screen.getByText(/相对系统 DPI/)).toBeTruthy();
});
