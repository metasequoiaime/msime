// @vitest-environment jsdom
import { StrictMode, useRef } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { AccountVerificationForm } from "../../../../packages/ui/src/account/account-verification-form";
import { useAccountVerification } from "../../../../packages/ui/src/account/use-account-verification";
import type { AccountClient } from "@msime/ui";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function VerificationForm({
  client,
  variant,
}: {
  client: Pick<AccountClient, "requestCode" | "login">;
  variant: "page" | "sheet";
}) {
  const mounted = useRef(true);
  const clientGeneration = useRef(1);
  const verification = useAccountVerification({
    client,
    mounted,
    clientGeneration,
    setNotice: () => {},
  });
  return (
    <>
      <button onClick={() => verification.selectChannel("email")}>使用邮箱登录</button>
      <button onClick={() => verification.selectChannel("phone")}>使用手机号登录</button>
      <AccountVerificationForm
        variant={variant}
        verification={verification}
        busy={false}
        onRequestCode={() => void verification.sendCode()}
        onSignIn={() => void verification.signIn()}
      />
    </>
  );
}

test.each([
  { variant: "page", change: "target" },
  { variant: "page", change: "channel" },
  { variant: "sheet", change: "target" },
  { variant: "sheet", change: "channel" },
] as const)(
  "$variant 表单更换 $change 后仍按原截止时间恢复验证码发送",
  async ({ variant, change }) => {
    const client = {
      requestCode: vi
        .fn()
        .mockResolvedValue({ challengeId: "synthetic-challenge", expiresIn: 300 }),
      login: vi.fn(),
    };
    const view = render(
      <StrictMode>
        <VerificationForm client={client} variant={variant} />
      </StrictMode>,
    );
    expect(vi.getTimerCount()).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "使用邮箱登录" }));
    fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
      target: { value: "synthetic@example.test" },
    });
    await act(async () => {
      fireEvent.click(
        screen.getByRole("button", { name: variant === "page" ? "获取验证码" : "发送验证码" }),
      );
    });
    expect(screen.getByRole("textbox", { name: "6 位验证码" })).not.toBeNull();
    act(() => vi.advanceTimersByTime(10_000));

    const nextChannel = change === "channel" ? "phone" : "email";
    const nextTarget = change === "channel" ? "+810000000001" : "next@example.test";
    if (change === "channel")
      fireEvent.click(screen.getByRole("button", { name: "使用手机号登录" }));
    fireEvent.change(
      screen.getByRole("textbox", {
        name: nextChannel === "email" ? "邮箱地址" : "手机号（含国家区号）",
      }),
      { target: { value: nextTarget } },
    );
    expect(screen.queryByRole("textbox", { name: "6 位验证码" })).toBeNull();
    expect(
      (screen.getByRole("button", { name: "50 秒后可重新发送" }) as HTMLButtonElement).disabled,
    ).toBe(true);

    act(() => vi.advanceTimersByTime(20_000));
    expect(
      (screen.getByRole("button", { name: "30 秒后可重新发送" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    act(() => vi.advanceTimersByTime(30_000));
    const send = screen.getByRole("button", {
      name: variant === "page" ? "获取验证码" : "发送验证码",
    }) as HTMLButtonElement;
    expect(send.disabled).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
    await act(async () => {
      fireEvent.click(send);
    });
    expect(client.requestCode).toHaveBeenLastCalledWith(nextChannel, nextTarget);
    expect(screen.getByRole("textbox", { name: "6 位验证码" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "60 秒后可重新发送" })).not.toBeNull();
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  },
);
