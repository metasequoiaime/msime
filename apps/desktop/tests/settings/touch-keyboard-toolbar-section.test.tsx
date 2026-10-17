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

test("鸿蒙工具栏给出显示方式和五个工具开关，改一项保留其他选择", () => {
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
    ["表情", "常用语", "剪贴板", "皮肤", "输入方式"].map(
      (label) =>
        (screen.getByRole("switch", { name: `工具栏：${label}` }) as HTMLInputElement).checked,
    ),
  ).toEqual([true, true, false, true, true]);
  fireEvent.click(screen.getByRole("switch", { name: "工具栏：常用语" }));
  expect(onHarmonyToolbarChange).toHaveBeenLastCalledWith({
    emoji: true,
    phrase: false,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: false,
  });
  fireEvent.change(screen.getByLabelText("工具栏显示方式"), { target: { value: "hidden" } });
  expect(onHarmonyToolbarChange).toHaveBeenLastCalledWith({
    emoji: true,
    phrase: true,
    clipboard: false,
    skin: true,
    scheme: true,
    hidden: true,
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
