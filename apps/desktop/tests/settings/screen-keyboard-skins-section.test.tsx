// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ScreenKeyboardSkinsSection, type TouchKeyboardSkinDesign } from "@msime/ui";

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

test("chooses the custom keyboard skin", () => {
  const onSelect = vi.fn();
  render(
    <ScreenKeyboardSkinsSection
      theme="light"
      selected={false}
      customDesign={customDesign}
      editorOpen={false}
      onSelect={onSelect}
      onToggleEditor={vi.fn()}
    />,
  );

  const card = screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" });
  expect(card.getAttribute("aria-checked")).toBe("false");
  fireEvent.click(card);
  expect(onSelect).toHaveBeenCalledOnce();
});

test("shows the selected custom skin and toggles its editor", () => {
  const onToggleEditor = vi.fn();
  render(
    <ScreenKeyboardSkinsSection
      theme="dark"
      selected
      customDesign={customDesign}
      editorOpen={false}
      onSelect={vi.fn()}
      onToggleEditor={onToggleEditor}
    />,
  );

  expect(
    screen.getByRole("switch", { name: "屏幕键盘皮肤 我的皮肤" }).getAttribute("aria-checked"),
  ).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "设计我的皮肤" }));
  expect(onToggleEditor).toHaveBeenCalledOnce();
});
