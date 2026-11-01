// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  defaultTouchKeyboardGeometry,
  touchKeyboardResetSubjects,
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

test.each([false, true])("keeps editable spacing bounds in phone layout %s", (phone) => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      layoutRows={phone ? <div>中文键盘</div> : undefined}
    />,
  );
  const key = screen.getByLabelText("按键间距") as HTMLInputElement;
  const row = screen.getByLabelText("行间距") as HTMLInputElement;
  expect([key.min, key.max, row.min, row.max]).toEqual(["30", "60", "40", "100"]);
  fireEvent.change(key, { target: { value: "0" } });
  fireEvent.change(row, { target: { value: "110" } });
  expect(baseProps.onKeySpacingChange).toHaveBeenLastCalledWith(30);
  expect(baseProps.onRowSpacingChange).toHaveBeenLastCalledWith(100);
});

test("forwards geometry, toolbar, feedback, and reset actions", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} />);

  fireEvent.change(screen.getByLabelText("键盘高度"), { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText("按键间距"), { target: { value: "55" } });
  fireEvent.change(screen.getByLabelText("行间距"), { target: { value: "80" } });
  fireEvent.click(screen.getByLabelText("顶部语音入口"));
  fireEvent.click(screen.getByLabelText("工具栏：表情"));
  fireEvent.click(screen.getByLabelText("数字行与 Tab 键"));
  fireEvent.click(screen.getByLabelText("分离式键盘"));
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
  expect(screen.queryByLabelText("分离式键盘")).toBeNull();
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

test("draws the letter corner hint switch only when the host gives it, and does not grey it with the swipe", () => {
  const onLetterCornerHintsChange = vi.fn();
  const { rerender } = render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      swipeSymbols={false}
      letterCornerHints={true}
      onLetterCornerHintsChange={onLetterCornerHintsChange}
    />,
  );
  const row = screen.getByLabelText("显示字母键角标") as HTMLInputElement;
  expect(row.checked).toBe(true);
  expect(row.disabled).toBe(false);
  fireEvent.click(row);
  expect(onLetterCornerHintsChange).toHaveBeenCalledWith(false);

  rerender(<TouchKeyboardGeometrySection {...baseProps} letterCornerHints={undefined} />);
  expect(screen.queryByLabelText("显示字母键角标")).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      glideTyping={undefined}
      swipeSymbols={undefined}
      letterCornerHints={false}
    />,
  );
  expect(screen.getByText("手势")).toBeTruthy();
  expect((screen.getByLabelText("显示字母键角标") as HTMLInputElement).checked).toBe(false);
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
  expect((screen.getByLabelText("分离式键盘") as HTMLInputElement).checked).toBe(true);
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
  // 没有「工具栏」组时说明不再提工具栏按钮。
  expect(screen.getByText("高度和间距回到默认")).toBeTruthy();
});

test("the reset names only what the page shows and hands the same words to the confirmation", () => {
  const onReset = vi.fn();
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      showHeight={false}
      voiceShortcutKind="start-voice"
      toolbarComponents={false}
      onReset={onReset}
    />,
  );
  // Linux：高度不起作用、没有工具栏按钮，只剩间距和顶部语音入口。
  expect(screen.queryByLabelText("键盘高度")).toBeNull();
  expect(screen.getByLabelText("按键间距")).toBeTruthy();
  expect(screen.getByText("间距和顶部语音入口回到默认")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "恢复屏幕键盘默认设置" }));
  expect(onReset).toHaveBeenCalledWith("间距和顶部语音入口");
});

test("a host that applies neither height nor spacing gets no size group at all", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      showHeight={false}
      showSpacing={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      glideTyping={undefined}
      swipeSymbols={undefined}
    />,
  );
  // macOS：键盘窗口高度固定、间距固定成默认值，尺寸组连同「恢复默认」一起不画。
  expect(screen.queryByText("尺寸")).toBeNull();
  expect(screen.queryByLabelText("键盘高度")).toBeNull();
  expect(screen.queryByLabelText("按键间距")).toBeNull();
  expect(screen.queryByLabelText("行间距")).toBeNull();
  expect(screen.queryByRole("button", { name: "恢复屏幕键盘默认设置" })).toBeNull();
});

test("touchKeyboardResetSubjects joins what is shown", () => {
  expect(
    touchKeyboardResetSubjects({ height: true, spacing: true, voiceShortcut: true, toolbar: true }),
  ).toBe("高度、间距、顶部语音入口和工具栏按钮");
  expect(
    touchKeyboardResetSubjects({
      height: true,
      spacing: true,
      voiceShortcut: true,
      toolbar: false,
    }),
  ).toBe("高度、间距和顶部语音入口");
  expect(
    touchKeyboardResetSubjects({
      height: false,
      spacing: true,
      voiceShortcut: false,
      toolbar: false,
    }),
  ).toBe("间距");
});

test("disables the tablet switch while saving", () => {
  render(<TouchKeyboardGeometrySection {...baseProps} tabletFullKeysBusy />);

  expect((screen.getByLabelText("数字行与 Tab 键") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("分离式键盘") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("滑行输入") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("滑动输入符号") as HTMLInputElement).disabled).toBe(true);
  expect((screen.getByLabelText("滑动方向") as HTMLSelectElement).disabled).toBe(true);
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
      shuangpinKeyHints={false}
      onShuangpinKeyHintsChange={vi.fn()}
      layoutRows={<div>中文键盘</div>}
    />,
  );
  // HarmonyOS 手机的尺寸组就叫「布局」，不再另起一个同名分组。
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const hints = screen.getByRole("switch", { name: "双拼键位提示" }) as HTMLInputElement;
  expect(hints.checked).toBe(false);
  expect(
    screen.getByText("中文键盘").compareDocumentPosition(hints) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    hints.compareDocumentPosition(screen.getByText("键盘高度")) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
});

test("offers the one-handed side number pad switch only where the host draws the pad", () => {
  const onOneHandedSidePanelChange = vi.fn();
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect(screen.queryByRole("switch", { name: "大屏单手时侧边显示数字键盘" })).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      oneHandedSidePanel={false}
      onOneHandedSidePanelChange={onOneHandedSidePanelChange}
    />,
  );
  // 只有这一行时也要有「布局」分组来放它；标题和说明与 Android、iOS 原生设置页逐字相同。
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const pad = screen.getByRole("switch", {
    name: "大屏单手时侧边显示数字键盘",
  }) as HTMLInputElement;
  expect(pad.checked).toBe(false);
  expect(
    screen.getByText(
      "单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。",
    ),
  ).toBeTruthy();
  fireEvent.click(pad);
  expect(onOneHandedSidePanelChange).toHaveBeenCalledWith(true);
});

test("puts the one-handed side number pad switch in the phone 布局 group with its description", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      tabletFullKeys={undefined}
      tabletSplitKeyboard={undefined}
      oneHandedSidePanel
      onOneHandedSidePanelChange={vi.fn()}
      layoutRows={<div>中文键盘</div>}
    />,
  );
  expect(screen.getAllByText("布局")).toHaveLength(1);
  const pad = screen.getByRole("switch", {
    name: "大屏单手时侧边显示数字键盘",
  }) as HTMLInputElement;
  expect(pad.checked).toBe(true);
  expect(
    screen.getByText("中文键盘").compareDocumentPosition(pad) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    pad.compareDocumentPosition(screen.getByText("键盘高度")) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  // 手机排法的其他行不带说明，这一行带：它只在平板和可折叠手机上出现，三端文字逐字相同。
  expect(
    screen.getByText(
      "单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。",
    ),
  ).toBeTruthy();
});

// 九键滑动输入数字（#10229）：三端同一行文字、同一组选项，紧跟 26 键的「滑动方向」。
test("lists 九键滑动输入数字 right after 滑动方向 and hands each choice's own spelling back", () => {
  const onNineKeySwipeDigitsChange = vi.fn();
  const { rerender } = render(<TouchKeyboardGeometrySection {...baseProps} />);
  expect(screen.queryByLabelText("九键滑动输入数字")).toBeNull();

  rerender(
    <TouchKeyboardGeometrySection
      {...baseProps}
      nineKeySwipeDigits="off"
      onNineKeySwipeDigitsChange={onNineKeySwipeDigitsChange}
    />,
  );
  const row = screen.getByLabelText("九键滑动输入数字") as HTMLSelectElement;
  expect(row.value).toBe("off");
  expect([...row.options].map((option) => [option.value, option.textContent])).toEqual([
    ["off", "关闭"],
    ["up", "上滑"],
    ["down", "下滑"],
  ]);
  expect(screen.getByText("沿所选方向滑动输入数字，反方向弹出字母。")).toBeTruthy();
  expect(
    screen.getByLabelText("滑动方向").compareDocumentPosition(row) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(
    row.compareDocumentPosition(screen.getByLabelText("滑行输入")) &
      Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  for (const value of ["up", "down", "off"] as const) {
    fireEvent.change(row, { target: { value } });
    expect(onNineKeySwipeDigitsChange).toHaveBeenLastCalledWith(value);
  }
});

test("九键滑动输入数字 does not follow the 26 key switch or the local save", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      swipeSymbols={false}
      tabletFullKeysBusy
      nineKeySwipeDigits="up"
      onNineKeySwipeDigitsChange={vi.fn()}
    />,
  );
  const row = screen.getByLabelText("九键滑动输入数字") as HTMLSelectElement;
  expect(row.value).toBe("up");
  expect(row.disabled).toBe(false);
});

test("draws the gestures group for 九键滑动输入数字 alone", () => {
  render(
    <TouchKeyboardGeometrySection
      {...baseProps}
      glideTyping={undefined}
      swipeSymbols={undefined}
      nineKeySwipeDigits="down"
      onNineKeySwipeDigitsChange={vi.fn()}
    />,
  );
  expect(screen.getByText("手势")).toBeTruthy();
  expect(screen.queryByLabelText("滑动输入符号")).toBeNull();
  expect((screen.getByLabelText("九键滑动输入数字") as HTMLSelectElement).value).toBe("down");
});
