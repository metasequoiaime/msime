// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { openKeyboardSwitchGuide } from "@msime/ui";
import { KeyboardSwitchGuideHost } from "../../../../packages/ui/src/keyboard/keyboard-switch-guide";
import type {
  ImeSetupClient,
  ImeSetupState,
} from "../../../../packages/ui/src/core/host-contracts";

afterEach(() => {
  vi.useRealTimers();
  cleanup();
});

function fakeSetup(first: ImeSetupState) {
  const listeners = new Set<(state: ImeSetupState) => void>();
  const client: ImeSetupClient = {
    read: () => first,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
  const push = (state: ImeSetupState) =>
    act(() => listeners.forEach((listener) => listener(state)));
  return { client, push };
}

// HarmonyOS 手机上「设为默认」不弹系统那个没有样式的选择对话框，改成引导：弹出键盘，告诉用户长按键盘左下角的 🌐 选水杉。
test("the guide raises the keyboard and tells where the switcher is", async () => {
  const setup = fakeSetup({ enabled: true, current: false });
  render(<KeyboardSwitchGuideHost setup={setup.client} />);
  expect(screen.queryByRole("dialog")).toBeNull();

  await act(() => openKeyboardSwitchGuide());
  const dialog = screen.getByRole("dialog", { name: "把水杉设为默认输入法" });
  expect(dialog.textContent).toContain("长按键盘左下角的");
  expect(dialog.textContent).toContain("「水杉输入法」");
  expect(document.activeElement).toBe(screen.getByRole("textbox", { name: "弹出键盘" }));

  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(screen.queryByRole("dialog")).toBeNull();
});

// 用户在系统菜单里选了水杉，宿主报告它成了当前输入法：引导说一声「已设为默认」，然后自己关掉。
test("the guide closes itself once 水杉 is the keyboard", async () => {
  vi.useFakeTimers();
  const setup = fakeSetup({ enabled: true, current: false });
  render(<KeyboardSwitchGuideHost setup={setup.client} />);
  await act(() => openKeyboardSwitchGuide());

  setup.push({ enabled: true, current: true });
  expect(screen.getByRole("status").textContent).toContain("已设为默认");
  act(() => {
    vi.advanceTimersByTime(1000);
  });
  expect(screen.queryByRole("dialog")).toBeNull();
});

// 从 🌐 菜单切换输入法时宿主收不到系统事件：引导开着就每秒请宿主重读一次，设好了就不再问。
test("the guide asks the host to re-read the setup until 水杉 is the keyboard", async () => {
  vi.useFakeTimers();
  const setup = fakeSetup({ enabled: true, current: false });
  const refresh = vi.fn();
  render(<KeyboardSwitchGuideHost setup={{ ...setup.client, refresh }} />);
  await act(() => openKeyboardSwitchGuide());

  act(() => {
    vi.advanceTimersByTime(3000);
  });
  expect(refresh).toHaveBeenCalledTimes(3);

  setup.push({ enabled: true, current: true });
  act(() => {
    vi.advanceTimersByTime(3000);
  });
  expect(refresh).toHaveBeenCalledTimes(3);
});
