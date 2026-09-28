// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { InputModeShortcutsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const keybindings = {
  switch_language_shift: true,
  switch_language_ctrl: false,
  switch_language_ctrl_alt_space: true,
  toggle_character_set_ctrl_shift_f: true,
  toggle_fullwidth_option_shift_h: true,
};

test("input mode shortcut changes report a keybinding patch", () => {
  const onChange = vi.fn();
  render(
    <InputModeShortcutsSection
      keybindings={keybindings}
      onChange={onChange}
      showModeSwitchShortcuts
      macos={false}
      showInputModeHUD={false}
      inputModeHUD={false}
      showFullwidthChord={false}
      windows={false}
    />,
  );

  fireEvent.click(screen.getByLabelText("Shift 切换中英文"));
  expect(onChange).toHaveBeenCalledWith({ switch_language_shift: false });
});

test("macOS input mode shortcut section exposes HUD and fullwidth controls", () => {
  const onChange = vi.fn();
  render(
    <InputModeShortcutsSection
      keybindings={keybindings}
      onChange={onChange}
      showModeSwitchShortcuts
      macos
      showInputModeHUD
      inputModeHUD={true}
      showFullwidthChord
      fullwidthChord="Option+Shift+H"
      windows={false}
    />,
  );

  expect(screen.getByLabelText("切换中英文时显示提示")).toBeTruthy();
  expect(screen.getByLabelText("Option+Shift+H 切换全半角")).toBeTruthy();
});
