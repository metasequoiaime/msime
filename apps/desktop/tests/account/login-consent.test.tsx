// @vitest-environment jsdom
import { accountProviderNames } from "../support/account-provider-names";
import { presentAccountName } from "../support/account-name";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage, type AccountClient, type AccountUser } from "@msime/ui";
import { LoginSheet } from "../../../../packages/ui/src/account/login-sheet";

afterEach(cleanup);

const user: AccountUser = {
  id: "synthetic-consent-user",
  displayName: "合成测试用户",
  preferredName: "合成测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};
const providers = { email: true, phone: false, google: true };
const consentName = "我已阅读并同意《用户协议》和《隐私政策》";
const prompt = "请先阅读并勾选同意《用户协议》和《隐私政策》";

function client(overrides: Partial<AccountClient> = {}): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user: null }),
    providers: vi.fn().mockResolvedValue(providers),
    requestCode: vi.fn().mockResolvedValue({ challengeId: "synthetic-challenge", expiresIn: 300 }),
    login: vi.fn().mockResolvedValue({ user }),
    profile: vi
      .fn()
      .mockResolvedValue({ user, providers: ["email"], providerNames: accountProviderNames }),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn().mockResolvedValue(undefined),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
    googleLogin: vi.fn().mockResolvedValue({ user }),
    googleCancel: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function sheet(account: AccountClient, onOpenUrl?: (url: string) => void) {
  const onSignedIn = vi.fn();
  render(
    <LoginSheet
      client={account}
      providers={providers}
      onProvidersChange={vi.fn()}
      onSignedIn={onSignedIn}
      onClearedExpired={vi.fn()}
      onClose={vi.fn()}
      onOpenUrl={onOpenUrl}
    />,
  );
  return { onSignedIn, dialog: within(screen.getByRole("dialog", { name: "登录水杉" })) };
}

test("鸿蒙登录面板的同意框默认不勾，也不再写「登录即表示同意」，《用户协议》能点开", () => {
  const { dialog } = sheet(client(), vi.fn());
  const checkbox = dialog.getByRole("checkbox", { name: consentName }) as HTMLInputElement;
  expect(checkbox.checked).toBe(false);
  expect(dialog.queryByText(/登录即/)).toBeNull();
  expect(dialog.getByRole("button", { name: "《用户协议》" })).not.toBeNull();
  expect(dialog.queryByRole("alert")).toBeNull();
});

test("鸿蒙登录面板没勾同意时发送验证码和 Google 登录都只提示、不发请求", async () => {
  const account = client();
  const { dialog } = sheet(account, vi.fn());
  const checkbox = dialog.getByRole("checkbox", { name: consentName }) as HTMLInputElement;

  fireEvent.click(dialog.getByRole("button", { name: "使用 Google 登录" }));
  expect(account.googleLogin).not.toHaveBeenCalled();
  expect(dialog.getByRole("alert").textContent).toBe(prompt);
  expect(document.activeElement).toBe(checkbox);
  expect(checkbox.getAttribute("aria-describedby")).toBe(dialog.getByRole("alert").id);

  fireEvent.click(dialog.getByRole("button", { name: "使用邮箱登录" }));
  fireEvent.change(dialog.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "synthetic@example.test" },
  });
  fireEvent.click(dialog.getByRole("button", { name: "发送验证码" }));
  expect(account.requestCode).not.toHaveBeenCalled();
  expect(dialog.getByRole("alert").textContent).toBe(prompt);

  // 勾上后提示消失，同一个按钮才真正发出请求。
  fireEvent.click(checkbox);
  expect(checkbox.checked).toBe(true);
  expect(dialog.queryByRole("alert")).toBeNull();
  fireEvent.click(dialog.getByRole("button", { name: "发送验证码" }));
  await waitFor(() =>
    expect(account.requestCode).toHaveBeenCalledWith("email", "synthetic@example.test"),
  );
});

test("鸿蒙登录面板验证码已发出后取消勾选，登录按钮同样被拦下", async () => {
  const account = client();
  const { dialog, onSignedIn } = sheet(account, vi.fn());
  const checkbox = dialog.getByRole("checkbox", { name: consentName });
  fireEvent.click(checkbox);
  fireEvent.click(dialog.getByRole("button", { name: "使用邮箱登录" }));
  fireEvent.change(dialog.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "synthetic@example.test" },
  });
  fireEvent.click(dialog.getByRole("button", { name: "发送验证码" }));
  fireEvent.change(await dialog.findByRole("textbox", { name: "6 位验证码" }), {
    target: { value: "123456" },
  });

  fireEvent.click(checkbox);
  fireEvent.click(dialog.getByRole("button", { name: "登录" }));
  expect(account.login).not.toHaveBeenCalled();
  expect(dialog.getByRole("alert").textContent).toBe(prompt);

  fireEvent.click(checkbox);
  fireEvent.click(dialog.getByRole("button", { name: "登录" }));
  await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(user));
});

test("点《用户协议》《隐私政策》各自打开对应网页，不改变勾选；宿主不能开网页时它们是普通文字", () => {
  const onOpenUrl = vi.fn();
  const { dialog } = sheet(client(), onOpenUrl);
  fireEvent.click(dialog.getByRole("button", { name: "《用户协议》" }));
  expect(onOpenUrl).toHaveBeenLastCalledWith("https://msime.app/terms/");
  fireEvent.click(dialog.getByRole("button", { name: "《隐私政策》" }));
  expect(onOpenUrl).toHaveBeenLastCalledWith("https://msime.app/privacy/");
  expect((dialog.getByRole("checkbox", { name: consentName }) as HTMLInputElement).checked).toBe(
    false,
  );
  cleanup();

  const plain = sheet(client()).dialog;
  expect(plain.queryByRole("button", { name: "《隐私政策》" })).toBeNull();
  expect(plain.queryByRole("button", { name: "《用户协议》" })).toBeNull();
  expect(plain.getByRole("checkbox", { name: consentName })).not.toBeNull();
});

test("桌面式账号页（macOS、Linux、鸿蒙 2in1）同样要先勾选同意，退出登录后要重新勾", async () => {
  const account = client();
  const onOpenUrl = vi.fn();
  render(<AccountPage client={account} onOpenUrl={onOpenUrl} />);
  const checkbox = (await screen.findByRole("checkbox", { name: consentName })) as HTMLInputElement;
  expect(checkbox.checked).toBe(false);

  fireEvent.click(screen.getByRole("button", { name: "使用 Google 登录" }));
  expect(account.googleLogin).not.toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).toBe(prompt);

  fireEvent.click(screen.getByRole("button", { name: "邮箱登录" }));
  fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "synthetic@example.test" },
  });
  fireEvent.click(screen.getByRole("button", { name: "获取验证码" }));
  expect(account.requestCode).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "《隐私政策》" }));
  expect(onOpenUrl).toHaveBeenCalledWith("https://msime.app/privacy/");

  fireEvent.click(screen.getByRole("checkbox", { name: consentName }));
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "获取验证码" }));
  fireEvent.change(await screen.findByRole("textbox", { name: "6 位验证码" }), {
    target: { value: "123456" },
  });
  fireEvent.click(screen.getByRole("button", { name: "登录" }));
  await screen.findByRole("button", { name: "编辑个人资料" });

  fireEvent.click(screen.getByRole("button", { name: "退出登录" }));
  await waitFor(() => expect(account.logout).toHaveBeenCalledWith(false));
  expect(
    ((await screen.findByRole("checkbox", { name: consentName })) as HTMLInputElement).checked,
  ).toBe(false);
});
