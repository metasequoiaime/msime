// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const initial: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

async function openAppearance(platform: string, host: Record<string, unknown> = {}) {
  render(
    <SettingsPage
      initialPage="appearance"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn(),
        host: { platform, ...host } as never,
        home: { openKeyboard: vi.fn(), openSystemKeyboardSettings: vi.fn() },
      }}
    />,
  );
  // A touch host calls the same page 候选栏.
  await screen.findByRole("heading", { name: /^候选(窗口|栏)$/ });
}

// The Android keyboard resolves both of these against its own surface setting.
test("Android offers the emoji and handwriting panel themes", async () => {
  await openAppearance("android");

  expect(screen.getByLabelText("表情面板主题")).toBeTruthy();
  expect(screen.getByLabelText("手写识别板主题")).toBeTruthy();
});

// Only the native desktop menus read menu_theme; a touch host draws no menu it applies to.
test("mobile hosts do not offer a menu theme", async () => {
  await openAppearance("android");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();

  cleanup();
  await openAppearance("ios");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();
});

test("the desktop keeps the menu theme", async () => {
  await openAppearance("macos");

  expect(screen.getByLabelText("菜单主题")).toBeTruthy();
});

// The IBus property menu and the Fcitx5 status menu are drawn by the desktop panel, which applies its own theme.
test("Linux does not offer a menu theme it cannot apply", async () => {
  await openAppearance("linux");
  expect(screen.queryByLabelText("菜单主题")).toBeNull();
});

// Linux stands its toolbar up as the IBus property menu and the Fcitx5 status menu, drawn by the desktop panel; no Linux host reads toolbar_theme.
test("Linux does not offer a floating-toolbar theme it cannot apply", async () => {
  await openAppearance("linux", { floating_toolbar: true });
  expect(screen.queryByLabelText("悬浮工具栏主题")).toBeNull();
});

test("the desktop hosts that draw the toolbar keep its theme", async () => {
  for (const platform of ["macos", "windows"]) {
    await openAppearance(platform, { floating_toolbar: true });
    expect(screen.getByLabelText("悬浮工具栏主题")).toBeTruthy();
    cleanup();
  }
});
