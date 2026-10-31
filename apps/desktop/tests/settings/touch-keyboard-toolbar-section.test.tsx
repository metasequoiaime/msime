// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TouchKeyboardToolbarSection } from "../../../../packages/ui/src/settings/touch-keyboard-toolbar-section";

afterEach(cleanup);

test("工具栏分组可独立渲染，隐藏语音时仍能编辑按钮且保留其他选择", () => {
  const onToolbarChange = vi.fn();
  render(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents
      toolbar={Object.freeze({ emoji: false, ai: true })}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={onToolbarChange}
    />,
  );
  expect(screen.queryByRole("switch", { name: "顶部语音入口" })).toBeNull();
  expect(screen.getAllByRole("checkbox").map((item) => (item as HTMLInputElement).checked)).toEqual(
    [true, false, true, false, true, false, false, false],
  );
  fireEvent.click(screen.getByLabelText("工具栏：剪贴板历史"));
  expect(onToolbarChange).toHaveBeenCalledWith({
    layout: true,
    emoji: false,
    skin: true,
    clipboard: true,
    ai: true,
    character_set: false,
    fullwidth: false,
    punctuation: false,
  });
});

test("只有语音入口时按宿主描述，隐藏后不留下空分组", () => {
  const onTouchVoiceShortcutChange = vi.fn();
  const props = {
    touchVoiceShortcut: true,
    toolbarComponents: false,
    onTouchVoiceShortcutChange,
    onToolbarChange: vi.fn(),
  };
  const { rerender, container } = render(
    <TouchKeyboardToolbarSection {...props} voiceShortcutKind="last-result" />,
  );
  expect(screen.getByText("在触屏键盘工具栏直接打开最近一次语音结果")).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "顶部语音入口" }));
  expect(onTouchVoiceShortcutChange).toHaveBeenCalledWith(false);
  expect(screen.queryByRole("checkbox")).toBeNull();
  rerender(<TouchKeyboardToolbarSection {...props} voiceShortcutKind="hidden" />);
  expect(container.childElementCount).toBe(0);
});

test("鸿蒙工具栏给出显示方式、五个工具和随机数字开关，改一项保留其他选择", () => {
  const onHarmonyToolbarChange = vi.fn();
  const { rerender } = render(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      harmonyToolbar={Object.freeze({ clipboard: false })}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
      onHarmonyToolbarChange={onHarmonyToolbarChange}
    />,
  );
  // 不报告 `touch_toolbar_components` 的宿主不画 `touch_toolbar` 的勾选框。
  expect(screen.queryByRole("checkbox")).toBeNull();
  expect(
    ["表情", "常用语", "剪贴板", "皮肤", "输入方式", "随机数字"].map(
      (label) =>
        (screen.getByRole("switch", { name: `工具栏：${label}` }) as HTMLInputElement).checked,
    ),
  ).toEqual([true, true, false, true, true, false]);
  // 「随机数字」默认关，说明写清它只放出入口，点「随」才打乱。
  expect(
    screen.getByText("在中文工具栏显示“随”开关；点按后打乱数字九宫格，每输入一个数字重新排列"),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("switch", { name: "工具栏：常用语" }));
  expect(onHarmonyToolbarChange).toHaveBeenLastCalledWith({
    emoji: true,
    phrase: false,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: false,
    random_digits: false,
  });
  fireEvent.click(screen.getByRole("switch", { name: "工具栏：随机数字" }));
  expect(onHarmonyToolbarChange).toHaveBeenLastCalledWith({
    emoji: true,
    phrase: true,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: false,
    random_digits: true,
  });
  fireEvent.change(screen.getByLabelText("工具栏显示方式"), { target: { value: "hidden" } });
  expect(onHarmonyToolbarChange).toHaveBeenLastCalledWith({
    emoji: true,
    phrase: true,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: true,
    random_digits: false,
  });
  expect(screen.queryByText(/工具栏不显示|工具栏已隐藏/)).toBeNull();

  // 选了「隐藏」时工具开关不可用；五个工具全关时也说明整行不显示。
  rerender(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      harmonyToolbar={{ hidden: true }}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
      onHarmonyToolbarChange={onHarmonyToolbarChange}
    />,
  );
  expect((screen.getByRole("switch", { name: "工具栏：表情" }) as HTMLInputElement).disabled).toBe(
    true,
  );
  expect(
    (screen.getByRole("switch", { name: "工具栏：随机数字" }) as HTMLInputElement).disabled,
  ).toBe(true);
  expect(screen.getByText(/工具栏已隐藏/)).toBeTruthy();
  rerender(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      harmonyToolbar={{ emoji: false, phrase: false, clipboard: false, skin: false, scheme: false }}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
      onHarmonyToolbarChange={onHarmonyToolbarChange}
    />,
  );
  expect(screen.getByText(/没有打开的工具，工具栏不显示/)).toBeTruthy();
  // 只开着「随」时中文键盘这一行照常显示，与键盘的 `TouchToolbarPolicy.rowHidden` 相同。
  rerender(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      harmonyToolbar={{
        emoji: false,
        phrase: false,
        clipboard: false,
        skin: false,
        scheme: false,
        random_digits: true,
      }}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
      onHarmonyToolbarChange={onHarmonyToolbarChange}
    />,
  );
  expect(screen.queryByText(/工具栏不显示|工具栏已隐藏/)).toBeNull();
  // 日文等非中文方案的键盘不画「随」，只开着它时那一行照旧不显示，说明也照旧出现。
  rerender(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      harmonyToolbar={{
        emoji: false,
        phrase: false,
        clipboard: false,
        skin: false,
        scheme: false,
        random_digits: true,
      }}
      harmonyChineseScheme={false}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
      onHarmonyToolbarChange={onHarmonyToolbarChange}
    />,
  );
  expect(screen.getByText(/没有打开的工具，工具栏不显示/)).toBeTruthy();
});

test("没有鸿蒙工具栏能力的宿主不画显示方式", () => {
  const { container } = render(
    <TouchKeyboardToolbarSection
      touchVoiceShortcut={false}
      voiceShortcutKind="hidden"
      toolbarComponents={false}
      onTouchVoiceShortcutChange={vi.fn()}
      onToolbarChange={vi.fn()}
    />,
  );
  expect(container.childElementCount).toBe(0);
});
