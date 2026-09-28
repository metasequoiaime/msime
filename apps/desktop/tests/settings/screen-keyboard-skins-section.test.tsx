// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  ScreenKeyboardSkinsSection,
  type TouchKeyboardSkin,
  type TouchKeyboardSkinDesign,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const customDesign: TouchKeyboardSkinDesign = {
  background: 0xe8f0eb,
  keyBackground: 0xffffff,
  keyForeground: 0x17251d,
  accent: 0x185c47,
  actionBackground: 0x185c47,
  cornerRadius: 8,
  borderWidth: 0,
  shadow: 0,
  pattern: 0,
  monospaced: false,
};

test("selects a built-in keyboard skin", () => {
  const onSelect = vi.fn<(skin: TouchKeyboardSkin) => void>();
  render(
    <ScreenKeyboardSkinsSection
      mobile
      theme="light"
      selected="forest"
      customDesign={customDesign}
      customAvailable={false}
      editorOpen={false}
      onSelect={onSelect}
      onToggleEditor={vi.fn()}
    />,
  );

  fireEvent.click(screen.getByRole("switch", { name: "屏幕键盘皮肤 海盐蓝" }));
  expect(onSelect).toHaveBeenCalledWith("ocean");
  expect(screen.queryByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" })).toBeNull();
});

test("shows custom skin and toggles its editor", () => {
  const onToggleEditor = vi.fn();
  render(
    <ScreenKeyboardSkinsSection
      mobile={false}
      theme="dark"
      selected="custom"
      customDesign={customDesign}
      customAvailable
      editorOpen={false}
      onSelect={vi.fn()}
      onToggleEditor={onToggleEditor}
    />,
  );

  expect(screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  expect(onToggleEditor).toHaveBeenCalledOnce();
});
