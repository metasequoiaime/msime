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
  swipeSymbols: true,
  swipeSymbolsDirection: "down" as const,
  tabletFullKeysBusy: false,
  onHeightAdjustmentChange: vi.fn(),
  onKeySpacingChange: vi.fn(),
  onRowSpacingChange: vi.fn(),
  onTouchVoiceShortcutChange: vi.fn(),
  onToolbarChange: vi.fn(),
  onTabletFullKeysChange: vi.fn(),
  onTabletSplitKeyboardChange: vi.fn(),
  onGlideTypingChange: vi.fn(),
  onSwipeSymbolsChange: vi.fn(),
  onSwipeSymbolsDirectionChange: vi.fn(),
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
  fireEvent.click(screen.getByLabelText("滑动输入符号"));
  fireEvent.change(screen.getByLabelText("滑动方向"), { target: { value: "up" } });
  fireEvent.click(screen.getByRole("button", { name: "恢复屏幕键盘默认设置" }));

  expect(baseProps.onHeightAdjustmentChange).toHaveBeenCalledWith(12);
  expect(baseProps.onKeySpacingChange).toHaveBeenCalledWith(55);
  expect(baseProps.onRowSpacingChange).toHaveBeenCalledWith(80);
  expect(baseProps.onTouchVoiceShortcutChange).toHaveBeenCalledWith(true);
  expect(baseProps.onToolbarChange).toHaveBeenCalledWith({ ...toolbar, emoji: false });
  expect(baseProps.onTabletFullKeysChange).toHaveBeenCalledWith(false);
  expect(baseProps.onTabletSplitKeyboardChange).toHaveBeenCalledWith(true);
  expect(baseProps.onGlideTypingChange).toHaveBeenCalledWith(true);
  expect(baseProps.onSwipeSymbolsChange).toHaveBeenCalledWith(false);
  expect(baseProps.onSwipeSymbolsDirectionChange).toHaveBeenCalledWith("up");
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
      swipeSymbols={undefined}
    />,
  );

  expect(screen.queryByLabelText("工具栏：表情")).toBeNull();
  expect(screen.queryByLabelText("数字行与 Tab 键")).toBeNull();
  expect(screen.queryByLabelText("横屏分离式键盘")).toBeNull();
  expect(screen.queryByText("布局")).toBeNull();
  expect(screen.queryByLabelText("滑行输入")).toBeNull();
  expect(screen.queryByLabelText("滑动输入符号")).toBeNull();
  expect(screen.queryByLabelText("滑动方向")).toBeNull();
  expect(screen.queryByText("手势")).toBeNull();
});

test("greys out the swipe direction while swiping for symbols is off", () => {
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect((screen.getByLabelText("滑动方向") as HTMLSelectElement).disabled).toBe(false);

  rerender(<TouchKeyboardGeometrySection {...baseProps} swipeSymbols={false} />);
  expect((screen.getByLabelText("滑动输入符号") as HTMLInputElement).checked).toBe(false);
  expect((screen.getByLabelText("滑动方向") as HTMLSelectElement).disabled).toBe(true);
});

test("draws the gestures group for the swipe alone", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} glideTyping={undefined} />);

  expect(screen.getByText("手势")).toBeTruthy();
  expect(screen.queryByLabelText("滑行输入")).toBeNull();
  expect(screen.getByLabelText("滑动输入符号")).toBeTruthy();
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
  expect((screen.getByLabelText("滑动输入符号") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("滑动方向") as HTMLSelectElement).disabled).toBe(true);
});

test("offers the number keypad order only where the host has a nine-key digit layer", () => {
  const onNumberKeypadOrderChange = vi.fn();
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect(screen.queryByText("数字键盘顺序")).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      numberKeypadOrder="phone"
      onNumberKeypadOrderChange={onNumberKeypadOrderChange}
    />,
  );
  expect(screen.getByText("数字键盘顺序")).toBeTruthy();
  expect(screen.getByRole("radio", { name: "电话" })).toHaveProperty("checked", true);
  fireEvent.click(screen.getByRole("radio", { name: "计算器" }));
  expect(onNumberKeypadOrderChange).toHaveBeenCalledWith("calculator");
});

test("offers the 26-key number layout next to the keypad order when the host has it", () => {
  const onTwentySixKeyNumberLayoutChange = vi.fn();
  const { rerender } = render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      numberKeypadOrder="phone"
      onNumberKeypadOrderChange={vi.fn()}
    />,
  );
  expect(screen.queryByText("26 键数字键盘")).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      numberKeypadOrder="phone"
      onNumberKeypadOrderChange={vi.fn()}
      twentySixKeyNumberLayout="row"
      onTwentySixKeyNumberLayoutChange={onTwentySixKeyNumberLayoutChange}
    />,
  );
  const layoutRow = screen.getByText("26 键数字键盘");
  expect(
    layoutRow.compareDocumentPosition(screen.getByText("数字键盘顺序")) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(screen.getByRole("radio", { name: "一行" })).toHaveProperty("checked", true);
  fireEvent.click(screen.getByRole("radio", { name: "九宫格" }));
  expect(onTwentySixKeyNumberLayoutChange).toHaveBeenCalledWith("nine_key");
});

test("puts the number keypad order after 中文键盘 when the size group is already 布局", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      numberKeypadOrder="calculator"
      onNumberKeypadOrderChange={vi.fn()}
      layoutRows={<div>中文键盘</div>}
    />,
  );
  // HarmonyOS 手机的尺寸组就叫「布局」，不再另起一个同名分组。
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const layoutRow = screen.getByText("中文键盘");
  const orderRow = screen.getByText("数字键盘顺序");
  expect(
    layoutRow.compareDocumentPosition(orderRow) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    orderRow.compareDocumentPosition(screen.getByText("键盘高度")) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(screen.getByRole("radio", { name: "计算器" })).toHaveProperty("checked", true);
});

test("offers the shuangpin key hint switch only where the host draws the hints", () => {
  const onShuangpinKeyHintsChange = vi.fn();
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect(screen.queryByRole("switch", { name: "双拼键位提示" })).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      shuangpinKeyHints
      onShuangpinKeyHintsChange={onShuangpinKeyHintsChange}
    />,
  );
  // 只有这一行时也要有「布局」分组来放它。
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const hints = screen.getByRole("switch", { name: "双拼键位提示" }) as HTMLInputElement;
  expect(hints.checked).toBe(true);
  fireEvent.click(hints);
  expect(onShuangpinKeyHintsChange).toHaveBeenCalledWith(false);
});

test("puts the shuangpin key hint switch in the size group when that group is already 布局", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      numberKeypadOrder="phone"
      shuangpinKeyHints={false}
      onShuangpinKeyHintsChange={vi.fn()}
      layoutRows={<div>中文键盘</div>}
    />,
  );
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const hints = screen.getByRole("switch", { name: "双拼键位提示" }) as HTMLInputElement;
  expect(hints.checked).toBe(false);
  expect(
    screen.getByText("数字键盘顺序").compareDocumentPosition(hints) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    hints.compareDocumentPosition(screen.getByText("键盘高度")) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
});
