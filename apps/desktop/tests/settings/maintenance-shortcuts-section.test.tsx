// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { MaintenanceShortcutsSection } from "@msime/ui";

afterEach(cleanup);

test("maintenance shortcuts render Linux service actions", () => {
  render(
    <MaintenanceShortcutsSection visible macos={false} linux maintenanceChord="Ctrl+Shift+Alt" />,
  );

  expect(screen.getByText("全局维护快捷键")).toBeTruthy();
  expect(screen.getByText("清除当前输入法会话的 Engine 缓存")).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Alt+R")).toBeTruthy();
});

test("maintenance shortcuts use the macOS input-context wording", () => {
  render(
    <MaintenanceShortcutsSection
      visible
      macos
      linux={false}
      maintenanceChord="Ctrl+Shift+Option"
    />,
  );

  expect(screen.getByText("输入上下文维护快捷键")).toBeTruthy();
  expect(screen.getByText("重新注册并重启当前输入法")).toBeTruthy();
  expect(screen.getByText("Ctrl+Shift+Option+T")).toBeTruthy();
});
