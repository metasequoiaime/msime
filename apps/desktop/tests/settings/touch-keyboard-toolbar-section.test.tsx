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
