// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  defaultTouchKeyboardGeometry,
  TouchKeyboardGeometrySection,
  type TouchToolbarPreferences,
} from "@msime/ui";

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
  tabletSplitKeyboard: false,
  glideTyping: false,
  tabletFullKeysBusy: false,
  onHeightAdjustmentChange: vi.fn(),
  onKeySpacingChange: vi.fn(),
  onRowSpacingChange: vi.fn(),
  onTouchVoiceShortcutChange: vi.fn(),
  onToolbarChange: vi.fn(),
  onTabletFullKeysChange: vi.fn(),
  onTabletSplitKeyboardChange: vi.fn(),
  onGlideTypingChange: vi.fn(),
  onReset: vi.fn(),
};

test("publishes the shared touch keyboard geometry defaults", () => {
  expect(defaultTouchKeyboardGeometry).toEqual({
    heightAdjustment: 0,
    keySpacingTenths: 60,
    rowSpacingTenths: 70,
  });
});

test("forwards geometry, toolbar, feedback, and reset actions", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} />);

  fireEvent.change(screen.getByLabelText("键盘高度"), { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText("按键间距"), { target: { value: "55" } });
  fireEvent.change(screen.getByLabelText("行间距"), { target: { value: "80" } });
  fireEvent.click(screen.getByLabelText("顶部语音入口"));
  fireEvent.click(screen.getByLabelText("工具栏：表情"));
  fireEvent.click(screen.getByLabelText("数字行与 Tab 键"));
  fireEvent.click(screen.getByLabelText("横屏分离式键盘"));
  fireEvent.click(screen.getByLabelText("滑行输入"));
  fireEvent.click(screen.getByRole("button", { name: "恢复屏幕键盘默认设置" }));

  expect(baseProps.onHeightAdjustmentChange).toHaveBeenCalledWith(12);
  expect(baseProps.onKeySpacingChange).toHaveBeenCalledWith(55);
  expect(baseProps.onRowSpacingChange).toHaveBeenCalledWith(80);
  expect(baseProps.onTouchVoiceShortcutChange).toHaveBeenCalledWith(true);
  expect(baseProps.onToolbarChange).toHaveBeenCalledWith({ ...toolbar, emoji: false });
  expect(baseProps.onTabletFullKeysChange).toHaveBeenCalledWith(false);
  expect(baseProps.onTabletSplitKeyboardChange).toHaveBeenCalledWith(true);
  expect(baseProps.onGlideTypingChange).toHaveBeenCalledWith(true);
  expect(baseProps.onReset).toHaveBeenCalledOnce();
});

test("hides host-specific options when unavailable", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      toolbarComponents={false}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      glideTyping={undefined}
    />,
  );

  expect(screen.queryByLabelText("工具栏：表情")).toBeNull();
  expect(screen.queryByLabelText("数字行与 Tab 键")).toBeNull();
  expect(screen.queryByLabelText("横屏分离式键盘")).toBeNull();
  expect(screen.queryByText("布局")).toBeNull();
  expect(screen.queryByLabelText("滑行输入")).toBeNull();
  expect(screen.queryByText("手势")).toBeNull();
});

// 两个 iPad 开关各自按宿主是否给出决定显示与否。
test("shows the split keyboard switch on its own when only it is reported", () => {
  render(
    <TouchKeyboardGeometrySection {...baseProps} tabletFullKeys={undefined} tabletSplitKeyboard />,
  );

  expect(screen.queryByLabelText("数字行与 Tab 键")).toBeNull();
  expect((screen.getByLabelText("横屏分离式键盘") as HTMLInputElement).checked).toBe(true);
});

test("describes the voice button by what the host's button does", () => {
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect(screen.getByText("在键盘顶部显示一个语音按钮，点按开始语音输入")).toBeTruthy();
  expect(screen.getByText("高度、间距、顶部语音入口和工具栏按钮回到默认")).toBeTruthy();

  rerender(<TouchKeyboardGeometrySection {...baseProps} voiceShortcutKind="last-result" />);
  expect(screen.getByText("在触屏键盘工具栏直接打开最近一次语音结果")).toBeTruthy();
});

test("drops the voice switch, and the empty toolbar group, where the keyboard draws no voice button", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
    />,
  );

  expect(screen.queryByLabelText("顶部语音入口")).toBeNull();
  expect(screen.queryByText("工具栏")).toBeNull();
  expect(screen.getByText("高度、间距和工具栏按钮回到默认")).toBeTruthy();
});

test("disables the tablet switch while saving", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} tabletFullKeysBusy />);

  expect((screen.getByLabelText("数字行与 Tab 键") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("横屏分离式键盘") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("滑行输入") as HTMLInputElement).disabled).toBe(true);
});
