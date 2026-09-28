// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { PanelShortcutsSection } from "@msime/ui";

afterEach(cleanup);

test("panel shortcut section uses the macOS Command label", () => {
  render(<PanelShortcutsSection visible macos harmony={false} />);

  expect(screen.getByRole("group", { name: "面板快捷键" })).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Command+K")).toBeTruthy();
  expect(screen.getByText("可从当前输入上下文使用 Command 组合键打开面板。")).toBeTruthy();
});

test("panel shortcut section explains HarmonyOS physical keyboard usage", () => {
  render(<PanelShortcutsSection visible={true} macos={false} harmony />);

  expect(screen.getByText("连接实体键盘后，在输入状态下可用 Super 组合键打开面板。")).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Super+K")).toBeTruthy();
});
