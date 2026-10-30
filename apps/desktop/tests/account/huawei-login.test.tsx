// @vitest-environment jsdom
import { accountProviderNames } from "../support/account-provider-names";
import { presentAccountName } from "../support/account-name";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  AccountPage,
  type AccountClient,
  type AccountProviders,
  type AccountUser,
} from "@msime/ui";
import { LoginSheet } from "../../../../packages/ui/src/account/login-sheet";

afterEach(cleanup);

const user: AccountUser = {
  id: "synthetic-huawei-user",
  displayName: "合成华为用户",
  preferredName: "合成华为用户",
  createdAt: "2026-01-01T00:00:00Z",
};
const consentName = "我已阅读并同意《用户协议》和《隐私政策》";
const prompt = "请先阅读并勾选同意《用户协议》和《隐私政策》";
/** 服务端两种第三方登录都开着；按钮出不出现只看宿主提供了哪一个。 */
const bothEnabled: AccountProviders = { email: true, phone: false, google: true, huawei: true };

function baseClient(overrides: Partial<AccountClient> = {}): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user: null }),
    providers: vi.fn().mockResolvedValue(bothEnabled),
    requestCode: vi.fn().mockResolvedValue({ challengeId: "synthetic-challenge", expiresIn: 300 }),
    login: vi.fn().mockResolvedValue({ user }),
    profile: vi
      .fn()
      .mockResolvedValue({ user, providers: ["huawei"], providerNames: accountProviderNames }),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn().mockResolvedValue(undefined),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
    ...overrides,
  };
}

/** 鸿蒙宿主的形状（apps/harmony/src/main.tsx）：提供 huaweiLogin，不提供 googleLogin。 */
function harmonyClient(overrides: Partial<AccountClient> = {}): AccountClient {
  return baseClient({ huaweiLogin: vi.fn().mockResolvedValue({ user }), ...overrides });
}

/** 桌面宿主的形状：提供 Google 回环登录，没有 Account Kit。 */
function desktopClient(): AccountClient {
  return baseClient({
    googleLogin: vi.fn().mockResolvedValue({ user }),
    googleCancel: vi.fn().mockResolvedValue(undefined),
  });
}

function sheet(account: AccountClient, providers: AccountProviders = bothEnabled) {
  const onSignedIn = vi.fn();
  render(
    <LoginSheet
      client={account}
      providers={providers}
      onProvidersChange={vi.fn()}
      onSignedIn={onSignedIn}
      onClearedExpired={vi.fn()}
      onClose={vi.fn()}
    />,
  );
  return { onSignedIn, dialog: within(screen.getByRole("dialog", { name: "登录水杉" })) };
}

test("鸿蒙登录面板给出「华为账号登录」，不再给 Google 登录", () => {
  const { dialog } = sheet(harmonyClient());
  const button = dialog.getByRole("button", { name: "华为账号登录" });
  // 按华为账号登录按钮规范，按钮里带华为标志：浅色主题的红色版和深色主题的白色版各一张，都只是装饰，不改按钮的名字。
  const logos = button.querySelectorAll("img");
  expect(logos).toHaveLength(2);
  logos.forEach((logo) => expect(logo.getAttribute("alt")).toBe(""));
  // 邮箱登录同样是图标加文字，与华为账号那一行对齐。
  expect(
    dialog.getByRole("button", { name: "使用邮箱登录" }).querySelector("svg[aria-hidden='true']"),
  ).not.toBeNull();
  expect(dialog.queryByRole("button", { name: "使用 Google 登录" })).toBeNull();
  expect(dialog.queryByText("当前没有可用的登录方式，请稍后重试。")).toBeNull();
});

test("华为账号登录与其他登录方式走同一个同意勾选，勾上之后才拉起授权", async () => {
  const account = harmonyClient();
  const { dialog, onSignedIn } = sheet(account);
  const checkbox = dialog.getByRole("checkbox", { name: consentName });

  fireEvent.click(dialog.getByRole("button", { name: "华为账号登录" }));
  expect(account.huaweiLogin).not.toHaveBeenCalled();
  expect(dialog.getByRole("alert").textContent).toBe(prompt);
  expect(document.activeElement).toBe(checkbox);

  fireEvent.click(checkbox);
  fireEvent.click(dialog.getByRole("button", { name: "华为账号登录" }));
  await waitFor(() => expect(onSignedIn).toHaveBeenCalledWith(user));
  expect(account.huaweiLogin).toHaveBeenCalledTimes(1);
});

test("服务端没开华为账号时鸿蒙面板不给按钮；只剩它一种时也会提示没有可用的登录方式", () => {
  sheet(harmonyClient(), { email: true, phone: false, huawei: false });
  expect(screen.queryByRole("button", { name: "华为账号登录" })).toBeNull();
  cleanup();

  const { dialog } = sheet(harmonyClient(), { email: false, phone: false, huawei: false });
  expect(dialog.getByText("当前没有可用的登录方式，请稍后重试。")).not.toBeNull();
  cleanup();

  // 华为账号是唯一一种时不算「没有可用的登录方式」。
  const only = sheet(harmonyClient(), { email: false, phone: false, huawei: true }).dialog;
  expect(only.getByRole("button", { name: "华为账号登录" })).not.toBeNull();
  expect(only.queryByText("当前没有可用的登录方式，请稍后重试。")).toBeNull();
});

test("华为账号授权被取消或失败时面板留着，提示走账号页的错误文案", async () => {
  const account = harmonyClient({
    huaweiLogin: vi.fn().mockRejectedValue({ code: "account_network" }),
  });
  const { dialog, onSignedIn } = sheet(account);
  fireEvent.click(dialog.getByRole("checkbox", { name: consentName }));
  fireEvent.click(dialog.getByRole("button", { name: "华为账号登录" }));
  expect(await dialog.findByText("网络连接失败，请检查网络后重试。")).not.toBeNull();
  expect(onSignedIn).not.toHaveBeenCalled();
});

test("别的平台不变：没有 huaweiLogin 的宿主即使服务端开着华为账号也不给按钮，Google 照旧", () => {
  const { dialog } = sheet(desktopClient());
  expect(dialog.queryByRole("button", { name: "华为账号登录" })).toBeNull();
  expect(dialog.getByRole("button", { name: "使用 Google 登录" })).not.toBeNull();
});

test("桌面式账号页（鸿蒙 2in1）同样给出华为账号登录、要先勾选同意，登录后读个人资料", async () => {
  const account = harmonyClient();
  render(<AccountPage client={account} />);
  const button = await screen.findByRole("button", { name: "华为账号登录" });
  expect(button.querySelectorAll("img")).toHaveLength(2);
  expect(screen.queryByRole("button", { name: "使用 Google 登录" })).toBeNull();

  fireEvent.click(button);
  expect(account.huaweiLogin).not.toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).toBe(prompt);

  fireEvent.click(screen.getByRole("checkbox", { name: consentName }));
  fireEvent.click(screen.getByRole("button", { name: "华为账号登录" }));
  await waitFor(() => expect(account.profile).toHaveBeenCalled());
  expect(await screen.findByText("登录成功。")).not.toBeNull();
});

test("桌面式账号页在 macOS、Linux 上不变：没有华为账号按钮，Google 按钮还在", async () => {
  render(<AccountPage client={desktopClient()} />);
  expect(await screen.findByRole("button", { name: "使用 Google 登录" })).not.toBeNull();
  expect(screen.queryByRole("button", { name: "华为账号登录" })).toBeNull();
});
