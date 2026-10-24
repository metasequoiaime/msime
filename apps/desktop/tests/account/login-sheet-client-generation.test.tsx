// @vitest-environment jsdom
import { presentAccountName } from "../support/account-name";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { LoginSheet } from "../../../../packages/ui/src/account/login-sheet";
import type { AccountClient, AccountUser } from "@msime/ui";

afterEach(cleanup);

const user: AccountUser = {
  id: "synthetic-google-user",
  displayName: "合成测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};
const providers = { email: true, phone: false, google: true };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

function client(overrides: Partial<AccountClient>): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user: null }),
    providers: vi.fn().mockResolvedValue(providers),
    requestCode: vi.fn().mockResolvedValue({ challengeId: "synthetic-challenge", expiresIn: 300 }),
    login: vi.fn().mockResolvedValue({ user }),
    profile: vi.fn().mockResolvedValue({ user, providers: ["google"] }),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
    ...overrides,
  };
}

function sheetView(strict: boolean) {
  const callbacks = {
    onProvidersChange: vi.fn(),
    onSignedIn: vi.fn(),
    onClearedExpired: vi.fn(),
    onClose: vi.fn(),
  };
  const sheet = (account: AccountClient) => {
    const content = <LoginSheet client={account} providers={providers} {...callbacks} />;
    return strict ? <StrictMode>{content}</StrictMode> : content;
  };
  return { sheet, ...callbacks };
}

test.each([false, true])(
  "换客户端后解除旧 Google 等待，验证码请求仍禁止关闭（StrictMode=%s）",
  async (strict) => {
    const oldLogin = deferred<{ user: AccountUser }>();
    const request = deferred<{ challengeId: string; expiresIn: number }>();
    const old = client({
      googleLogin: vi.fn(() => oldLogin.promise),
      googleCancel: vi.fn().mockResolvedValue(undefined),
    });
    const next = client({
      googleLogin: vi.fn(),
      googleCancel: vi.fn(),
      requestCode: vi.fn(() => request.promise),
    });
    const { sheet, onClose, onSignedIn } = sheetView(strict);
    const view = render(sheet(old));
    fireEvent.click(screen.getByRole("button", { name: "使用 Google 登录" }));
    await screen.findByRole("button", { name: "正在等待浏览器完成 Google 登录…" });

    view.rerender(sheet(next));
    expect(old.googleCancel).toHaveBeenCalledOnce();
    expect(next.googleCancel).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "取消 Google 登录" })).toBeNull();
    expect(
      (screen.getByRole("button", { name: "使用 Google 登录" }) as HTMLButtonElement).disabled,
    ).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "使用邮箱登录" }));
    fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
      target: { value: "synthetic@example.test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "发送验证码" }));
    await waitFor(() => expect(next.requestCode).toHaveBeenCalledOnce());
    expect((screen.getByRole("button", { name: "关闭" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => {
      oldLogin.resolve({ user });
      request.resolve({ challengeId: "synthetic-challenge", expiresIn: 300 });
    });
    expect(onSignedIn).not.toHaveBeenCalled();
    expect((screen.getByRole("button", { name: "关闭" }) as HTMLButtonElement).disabled).toBe(
      false,
    );
  },
);

test.each(["success", "failure"])(
  "旧 Google 请求的迟到 %s 不结束新客户端的等待",
  async (outcome) => {
    const oldLogin = deferred<{ user: AccountUser }>();
    const newLogin = deferred<{ user: AccountUser }>();
    const old = client({
      googleLogin: vi.fn(() => oldLogin.promise),
      googleCancel: vi.fn().mockResolvedValue(undefined),
    });
    const next = client({
      googleLogin: vi.fn(() => newLogin.promise),
      googleCancel: vi.fn().mockResolvedValue(undefined),
    });
    const { sheet, onSignedIn, onClose } = sheetView(true);
    const view = render(sheet(old));
    fireEvent.click(screen.getByRole("button", { name: "使用 Google 登录" }));
    await screen.findByRole("button", { name: "正在等待浏览器完成 Google 登录…" });
    view.rerender(sheet(next));
    fireEvent.click(screen.getByRole("button", { name: "使用 Google 登录" }));
    await waitFor(() => expect(next.googleLogin).toHaveBeenCalledOnce());

    await act(async () => {
      if (outcome === "success") oldLogin.resolve({ user });
      else oldLogin.reject({ code: "account_unavailable" });
    });
    expect(onSignedIn).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "正在等待浏览器完成 Google 登录…" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    expect(old.googleCancel).toHaveBeenCalledOnce();
    expect(next.googleCancel).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
    view.unmount();
    expect(next.googleCancel).toHaveBeenCalledOnce();
    await act(async () => newLogin.resolve({ user }));
    expect(onSignedIn).not.toHaveBeenCalled();
  },
);
