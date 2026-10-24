// @vitest-environment jsdom
import { presentAccountName } from "../support/account-name";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage, type AccountClient, type AccountUser } from "@msime/ui";

const clipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  if (clipboardDescriptor) {
    Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
  } else {
    Reflect.deleteProperty(navigator, "clipboard");
  }
});

const user: AccountUser = {
  id: "fixture-user-id",
  displayName: "合成测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};

test.each([undefined, "android"] as const)(
  "%s 账号页连续复制后从最后一次成功重新计时，卸载时清除提示计时器",
  async (platform) => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    const client: AccountClient = {
      status: async () => ({ user }),
      providers: async () => ({ email: true, phone: false }),
      requestCode: vi.fn(),
      login: vi.fn(),
      profile: async () => ({ user, providers: ["email"] }),
      namePresentation: presentAccountName,
      rename: vi.fn(),
      logout: vi.fn(),
      deleteAccount: vi.fn(),
      clearExpired: vi.fn(),
    };
    const { unmount } = render(<AccountPage client={client} platform={platform} />);
    fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
    const copy = await screen.findByRole("button", { name: "#FIXTUR" });
    vi.useFakeTimers();

    await act(async () => fireEvent.click(copy));
    expect(writeText).toHaveBeenLastCalledWith(user.id);
    expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy();
    act(() => vi.advanceTimersByTime(1000));
    await act(async () => fireEvent.click(copy));
    act(() => vi.advanceTimersByTime(800));
    expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy();
    act(() => vi.advanceTimersByTime(1000));
    expect(screen.getByRole("button", { name: "#FIXTUR" })).toBeTruthy();

    await act(async () => fireEvent.click(copy));
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  },
);
