// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TouchKeyboardGeometrySection, type TouchToolbarPreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const toolbar: TouchToolbarPreferences = {
  layout: true,
  emoji: true,
  skin: true,
  clipboard: false,
  ai: false,
  character_set: false,
  fullwidth: false,
  punctuation: false,
};

const baseProps = {
  heightAdjustment: 0,
  keySpacingTenths: 60,
  rowSpacingTenths: 70,
  touchVoiceShortcut: false,
  toolbarComponents: true,
  toolbar,
  tabletFullKeys: true,
  tabletFullKeysBusy: false,
  onHeightAdjustmentChange: vi.fn(),
  onKeySpacingChange: vi.fn(),
  onRowSpacingChange: vi.fn(),
  onTouchVoiceShortcutChange: vi.fn(),
  onToolbarChange: vi.fn(),
  onTabletFullKeysChange: vi.fn(),
  onReset: vi.fn(),
};

test("forwards geometry, toolbar, feedback, and reset actions", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} />);

  fireEvent.change(screen.getByLabelText("键盘高度"), { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText("按键间距"), { target: { value: "55" } });
  fireEvent.change(screen.getByLabelText("行间距"), { target: { value: "80" } });
  fireEvent.click(screen.getByLabelText("顶部语音入口"));
  fireEvent.click(screen.getByLabelText("工具栏：表情"));
  fireEvent.click(screen.getByLabelText("数字行与 Tab 键"));
  fireEvent.click(screen.getByRole("button", { name: "恢复屏幕键盘默认设置" }));

  expect(baseProps.onHeightAdjustmentChange).toHaveBeenCalledWith(12);
  expect(baseProps.onKeySpacingChange).toHaveBeenCalledWith(55);
  expect(baseProps.onRowSpacingChange).toHaveBeenCalledWith(80);
  expect(baseProps.onTouchVoiceShortcutChange).toHaveBeenCalledWith(true);
  expect(baseProps.onToolbarChange).toHaveBeenCalledWith({ ...toolbar, emoji: false });
  expect(baseProps.onTabletFullKeysChange).toHaveBeenCalledWith(false);
  expect(baseProps.onReset).toHaveBeenCalledOnce();
});

test("hides host-specific options when unavailable", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      toolbarComponents={false}
      tabletFullKeys={undefined}
    />,
  );

  expect(screen.queryByLabelText("工具栏：表情")).toBeNull();
  expect(screen.queryByLabelText("数字行与 Tab 键")).toBeNull();
});

test("disables the tablet switch while saving", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} tabletFullKeysBusy />);

  expect((screen.getByLabelText("数字行与 Tab 键") as HTMLInputElement).disabled).toBe(true);
});
