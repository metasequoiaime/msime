// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ThemeSettingsSection, type ThemePreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: ThemePreferences = {
  settings_theme: "follow",
  screen_keyboard_theme: "follow",
  candidate_theme: "follow",
  toolbar_theme: "follow",
  menu_theme: "follow",
  emoji_theme: "follow",
  handwriting_theme: "follow",
  voice_theme: "follow",
};

test("theme selectors report the changed preference key", () => {
  const onChange = vi.fn();
  render(
    <ThemeSettingsSection
      preferences={preferences}
      mobile={false}
      linux={false}
      floatingToolbar
      desktopPanels
      onChange={onChange}
    />,
  );

  fireEvent.change(screen.getByLabelText("候选窗口主题"), { target: { value: "light" } });
  expect(onChange).toHaveBeenCalledWith("candidate_theme", "light");
  fireEvent.change(screen.getByLabelText("屏幕键盘主题"), { target: { value: "dark" } });
  expect(onChange).toHaveBeenCalledWith("screen_keyboard_theme", "dark");
});

test("theme selectors hide menus and voice panels on a touch host", () => {
  render(
    <ThemeSettingsSection
      preferences={preferences}
      mobile
      linux={false}
      floatingToolbar
      desktopPanels={false}
      onChange={vi.fn()}
    />,
  );

  expect(screen.getByLabelText("候选栏主题")).toBeTruthy();
  expect(screen.getByLabelText("悬浮工具栏主题")).toBeTruthy();
  expect(screen.queryByLabelText("菜单主题")).toBeNull();
  expect(screen.queryByLabelText("语音输入弹出条主题")).toBeNull();
});
