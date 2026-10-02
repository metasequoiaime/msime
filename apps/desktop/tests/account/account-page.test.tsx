// @vitest-environment jsdom
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";
import { useState } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  AccountPage,
  SettingsPage,
  WelcomeFlowPage,
  type AccountClient,
  type AccountProfile,
  type AccountUser,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const user: AccountUser = {
  id: "fixture-user-id",
  displayName: "水杉测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};
const profile: AccountProfile = { user, providers: ["email"] };

function account(overrides: Partial<AccountClient> = {}): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user: null }),
    providers: vi.fn().mockResolvedValue({ email: true, phone: true }),
    requestCode: vi.fn().mockResolvedValue({ challengeId: "fixture-challenge", expiresIn: 300 }),
    login: vi.fn().mockResolvedValue({ user }),
    profile: vi.fn().mockResolvedValue(profile),
    rename: vi.fn().mockResolvedValue(profile),
    logout: vi.fn().mockResolvedValue(undefined),
    deleteAccount: vi.fn().mockResolvedValue(undefined),
    clearExpired: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

test("code login trims the target, requires six ASCII digits and loads the profile", async () => {
  const client = account();
  const onLoginComplete = vi.fn();
  render(<AccountPage client={client} onLoginComplete={onLoginComplete} />);
  fireEvent.click(await screen.findByRole("button", { name: "邮箱登录" }));
  fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "  fixture@example.test  " },
  });
  fireEvent.click(screen.getByRole("button", { name: "获取验证码" }));
  await waitFor(() =>
    expect(client.requestCode).toHaveBeenCalledWith("email", "fixture@example.test"),
  );
  const login = await screen.findByRole("button", { name: "登录" });
  expect((login as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByRole("textbox", { name: "6 位验证码" }), {
    target: { value: "12a３3456" },
  });
  expect((screen.getByRole("textbox", { name: "6 位验证码" }) as HTMLInputElement).value).toBe(
    "123456",
  );
  fireEvent.click(screen.getByRole("button", { name: "登录" }));
  await waitFor(() => expect(client.login).toHaveBeenCalledWith("fixture-challenge", "123456"));
  expect(await screen.findByText("水杉测试用户")).not.toBeNull();
  expect(onLoginComplete).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button", { name: "编辑个人资料" }));
  expect(
    within(screen.getByRole("dialog", { name: "编辑个人资料" })).getByText("邮箱"),
  ).not.toBeNull();
  expect(screen.queryByText("fixture-challenge")).toBeNull();
});

test("ignores a same-tick duplicate verification-code request", async () => {
  const pending = deferred<{ challengeId: string; expiresIn: number }>();
  const requestCode = vi.fn().mockReturnValue(pending.promise);
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    requestCode,
  });
  render(<AccountPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "邮箱登录" }));
  fireEvent.change(screen.getByRole("textbox", { name: "邮箱地址" }), {
    target: { value: "fixture@example.test" },
  });
  const request = screen.getByRole("button", { name: "获取验证码" });
  act(() => {
    fireEvent.click(request);
    fireEvent.click(request);
  });
  expect(requestCode).toHaveBeenCalledOnce();
  pending.resolve({ challengeId: "fixture-challenge", expiresIn: 300 });
  await waitFor(() => expect(screen.getByRole("textbox", { name: "6 位验证码" })).not.toBeNull());
});

test("profile rename, logout-all confirmation and account deletion use explicit actions", async () => {
  const renamed = { ...user, displayName: "新昵称" };
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    rename: vi.fn().mockResolvedValue({ user: renamed, providers: ["email"] }),
  });
  render(<AccountPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  // The profile dialog is the only place the nickname is edited; the page itself has no second nickname field.
  expect(screen.getAllByRole("textbox", { name: /社区昵称/ })).toHaveLength(1);
  const name = screen.getByRole("textbox", { name: "编辑社区昵称" });
  fireEvent.change(name, { target: { value: "  新昵称  " } });
  fireEvent.click(screen.getByRole("button", { name: "保存修改" }));
  await waitFor(() => expect(client.rename).toHaveBeenCalledWith("新昵称"));
  expect(await screen.findByText("昵称已更新。")).not.toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "退出所有设备" }));
  expect(screen.getByRole("alertdialog", { name: "确认退出所有设备" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(client.logout).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "退出所有设备" }));
  fireEvent.click(screen.getByRole("button", { name: "确认退出所有设备" }));
  await waitFor(() => expect(client.logout).toHaveBeenCalledWith(true));
});

test("a late profile mutation is ignored after the profile page unmounts", async () => {
  let finish!: (value: AccountProfile) => void;
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    rename: vi.fn(
      () =>
        new Promise<AccountProfile>((resolve) => {
          finish = resolve;
        }),
    ),
  });
  const view = render(<AccountPage client={client} platform="ios" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  fireEvent.change(await screen.findByRole("textbox", { name: "编辑社区昵称" }), {
    target: { value: "晚到的昵称" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存昵称" }));
  view.unmount();
  finish({ user: { ...user, displayName: "晚到的昵称" }, providers: ["email"] });
  await Promise.resolve();
});

test("a desktop profile rename response from a replaced client is ignored", async () => {
  let resolveOld!: (value: AccountProfile) => void;
  const oldClient = account({
    status: vi.fn().mockResolvedValue({ user }),
    rename: vi.fn(
      () =>
        new Promise<AccountProfile>((resolve) => {
          resolveOld = resolve;
        }),
    ),
  });
  const nextClient = account({
    status: vi.fn().mockResolvedValue({ user }),
  });
  const view = render(<AccountPage client={oldClient} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const name = screen.getByRole("textbox", { name: "编辑社区昵称" });
  fireEvent.change(name, { target: { value: "旧客户端昵称" } });
  fireEvent.click(screen.getByRole("button", { name: "保存修改" }));
  await waitFor(() => expect(oldClient.rename).toHaveBeenCalledWith("旧客户端昵称"));

  view.rerender(<AccountPage client={nextClient} />);
  await screen.findByRole("button", { name: "编辑个人资料" });
  await act(async () => {
    resolveOld({ user: { ...user, displayName: "响应旧昵称" }, providers: ["email"] });
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(screen.queryByText("响应旧昵称")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "编辑个人资料" }));
  expect(screen.queryByDisplayValue("响应旧昵称")).toBeNull();
});

test("mobile accounts keep profile editing and session actions on the pushed profile page", async () => {
  window.history.replaceState({ msimeSettings: true, page: "account" }, "");
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="ios" />);
  const profileCard = await screen.findByRole("button", { name: "编辑个人资料" });
  expect(screen.queryByRole("heading", { name: "个人资料" })).toBeNull();
  expect(screen.queryByRole("heading", { name: "账号" })).toBeNull();
  expect(screen.queryByText("账号操作")).toBeNull();
  fireEvent.click(profileCard);
  expect(await screen.findByRole("heading", { name: "编辑资料" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "重新登录" }));
  expect(screen.getByRole("alertdialog", { name: "确认重新登录" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "确认" }));
  await waitFor(() => expect(client.clearExpired).toHaveBeenCalledTimes(1));
});

test("mobile profile ignores a same-tick duplicate nickname save", async () => {
  const pending = deferred<AccountProfile>();
  const rename = vi.fn().mockReturnValue(pending.promise);
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    rename,
  });
  render(<AccountPage client={client} platform="ios" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const name = screen.getByRole("textbox", { name: "编辑社区昵称" });
  fireEvent.change(name, { target: { value: "移动端新昵称" } });
  const save = screen.getByRole("button", { name: "保存昵称" });
  act(() => {
    fireEvent.click(save);
    fireEvent.click(save);
  });
  expect(rename).toHaveBeenCalledOnce();
  pending.resolve({ user: { ...user, displayName: "移动端新昵称" }, providers: ["email"] });
  await waitFor(() => expect(screen.getByText("昵称已更新。")).not.toBeNull());
});

test("a mobile profile rename response from a replaced client is ignored", async () => {
  let resolveOld!: (value: AccountProfile) => void;
  const oldClient = account({
    status: vi.fn().mockResolvedValue({ user }),
    rename: vi.fn(() => new Promise<AccountProfile>((resolve) => (resolveOld = resolve))),
  });
  const nextClient = account({ status: vi.fn().mockResolvedValue({ user }) });
  const view = render(<AccountPage client={oldClient} platform="ios" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  fireEvent.change(await screen.findByRole("textbox", { name: "编辑社区昵称" }), {
    target: { value: "旧客户端昵称" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存昵称" }));
  await waitFor(() => expect(oldClient.rename).toHaveBeenCalled());
  view.rerender(<AccountPage client={nextClient} platform="ios" />);
  resolveOld({ user: { ...user, displayName: "响应旧昵称" }, providers: ["email"] });
  await Promise.resolve();
  expect(screen.queryByText("响应旧昵称")).toBeNull();
});

test("Harmony uses the mobile account flow instead of desktop account controls", async () => {
  window.history.replaceState({ msimeSettings: true, page: "account" }, "");
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="harmony" />);

  const profileCard = await screen.findByRole("button", { name: "编辑个人资料" });
  expect(screen.queryByRole("heading", { name: "个人资料" })).toBeNull();
  expect(screen.queryByText("账号操作")).toBeNull();
  fireEvent.click(profileCard);
  expect(await screen.findByRole("heading", { name: "编辑资料" })).not.toBeNull();
});

test("Harmony 2-in-1 uses desktop account controls even though its platform is Harmony", async () => {
  window.history.replaceState({ msimeSettings: true, page: "account" }, "");
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="harmony" mobile={false} />);

  expect(await screen.findByRole("heading", { name: "账号" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "编辑个人资料" }));
  expect(await screen.findByRole("dialog", { name: "编辑个人资料" })).not.toBeNull();
});

test("settings passes the Harmony 2-in-1 form factor into the account page", async () => {
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(
    <SettingsPage
      initialPage="account"
      client={{
        load: async () => preferences,
        save: vi.fn(),
        host: testHost({ platform: "harmony", mobile_settings: false }),
        account: client,
      }}
    />,
  );

  expect(await screen.findByRole("heading", { name: "账号" })).not.toBeNull();
  expect(screen.getByRole("button", { name: "编辑个人资料" })).not.toBeNull();
});

test("the mobile login sheet exposes its caller's cancel action", async () => {
  const onCancelLogin = vi.fn();
  render(<AccountPage client={account()} platform="harmony" onCancelLogin={onCancelLogin} />);

  expect(await screen.findByRole("heading", { name: "登录水杉" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onCancelLogin).toHaveBeenCalledOnce();
});

test("Harmony chat login focuses the tryout and cancel returns to it", async () => {
  window.history.replaceState({ msimeSettings: true, page: "chat" }, "");
  render(
    <SettingsPage
      initialPage="chat"
      client={{
        load: async () => preferences,
        save: vi.fn(),
        host: testHost({ platform: "harmony" }),
        home: {},
        chat: {
          models: vi.fn().mockRejectedValue({ code: "account_unauthorized" }),
          complete: vi.fn(),
        },
        account: account(),
      }}
    />,
  );

  const composer = await screen.findByRole("textbox", { name: "聊天消息" });
  await waitFor(() => expect(document.activeElement).toBe(composer));
  fireEvent.click(await screen.findByRole("button", { name: "登录使用 AI" }));
  expect(await screen.findByRole("heading", { name: "登录水杉" })).not.toBeNull();
  expect(screen.queryByText("账号操作")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(await screen.findByRole("button", { name: "登录使用 AI" })).not.toBeNull();
});

// Apple's account detail rows are 账号 ID, 登录方式 and 加入水杉, and the ID row is
// itself the copy button. The desktop editor had a case for that; the mobile
// page it pushes did not, so the parity was only true by inspection.
test("the mobile profile page copies the account ID and shows the join date", async () => {
  window.history.replaceState({ msimeSettings: true, page: "account" }, "");
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="android" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  await screen.findByRole("heading", { name: "编辑资料" });

  expect(screen.getByText("加入水杉")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "#FIXTUR" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith("fixture-user-id"));
  expect(await screen.findByRole("button", { name: "已复制" })).not.toBeNull();
});

test("profile card opens the shared editor and copies the complete account ID", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const dialog = screen.getByRole("dialog", { name: "编辑个人资料" });
  expect(within(dialog).getByText("加入水杉")).not.toBeNull();
  fireEvent.click(within(dialog).getByRole("button", { name: "#FIXTUR" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith("fixture-user-id"));
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(screen.queryByRole("dialog", { name: "编辑个人资料" })).toBeNull();
});

test("iOS Apple sign-in stays behind the native account client boundary", async () => {
  const appleLogin = vi.fn().mockResolvedValue({ user });
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, apple: true }),
    appleLogin,
  });
  render(<AccountPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Apple 登录" }));
  await waitFor(() => expect(appleLogin).toHaveBeenCalledTimes(1));
  expect(screen.queryByText(/token|nonce/i)).toBeNull();
});

test("an Apple sign-in response from a replaced account client is ignored", async () => {
  let resolveOld!: (value: { user?: AccountUser | null }) => void;
  const oldClient = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, apple: true }),
    appleLogin: vi.fn(
      () =>
        new Promise<{ user?: AccountUser | null }>((resolve) => {
          resolveOld = resolve;
        }),
    ),
  });
  const nextClient = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    status: vi.fn().mockResolvedValue({ user: null }),
  });
  const view = render(<AccountPage client={oldClient} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Apple 登录" }));
  await waitFor(() => expect(oldClient.appleLogin).toHaveBeenCalledTimes(1));

  view.rerender(<AccountPage client={nextClient} />);
  await screen.findByRole("button", { name: "邮箱登录" });
  await act(async () => {
    resolveOld({ user });
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(oldClient.profile).not.toHaveBeenCalled();
  expect(screen.queryByText("水杉测试用户")).toBeNull();
});

test("an Apple-only backend without a native Apple client shows the empty login state", async () => {
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, apple: true }),
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByText("当前没有可用的验证码登录方式，请稍后重试。")).not.toBeNull();
  expect(screen.queryByRole("button", { name: "使用 Apple 登录" })).toBeNull();
});

test("desktop Google sign-in runs through the native account client", async () => {
  const googleLogin = vi.fn().mockResolvedValue({ user });
  const onLoginComplete = vi.fn();
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false, google: true }),
    profile: vi.fn().mockResolvedValue({ user, providers: ["google"] }),
    googleLogin,
  });
  render(<AccountPage client={client} onLoginComplete={onLoginComplete} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Google 登录" }));
  await waitFor(() => expect(googleLogin).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(onLoginComplete).toHaveBeenCalledTimes(1));
  expect(client.profile).toHaveBeenCalled();
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  expect(
    await within(screen.getByRole("dialog", { name: "编辑个人资料" })).findByText("Google"),
  ).not.toBeNull();
  expect(client.requestCode).not.toHaveBeenCalled();
  expect(screen.queryByText(/token|code|state/i)).toBeNull();
});

test("a Google sign-in response from a replaced account client is ignored", async () => {
  let resolveOld!: (value: { user?: AccountUser | null }) => void;
  const oldClient = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, google: true }),
    googleLogin: vi.fn(
      () =>
        new Promise<{ user?: AccountUser | null }>((resolve) => {
          resolveOld = resolve;
        }),
    ),
  });
  const nextClient = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    status: vi.fn().mockResolvedValue({ user: null }),
  });
  const view = render(<AccountPage client={oldClient} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Google 登录" }));
  await waitFor(() => expect(oldClient.googleLogin).toHaveBeenCalledTimes(1));

  view.rerender(<AccountPage client={nextClient} />);
  await screen.findByRole("button", { name: "邮箱登录" });
  await act(async () => {
    resolveOld({ user });
    await Promise.resolve();
    await Promise.resolve();
  });
  expect(oldClient.profile).not.toHaveBeenCalled();
  expect(screen.queryByText("水杉测试用户")).toBeNull();
});

test("Google sign-in needs both the backend provider and a native Google client", async () => {
  const withoutClient = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, google: true }),
  });
  render(<AccountPage client={withoutClient} />);
  expect(await screen.findByText("当前没有可用的验证码登录方式，请稍后重试。")).not.toBeNull();
  expect(screen.queryByRole("button", { name: "使用 Google 登录" })).toBeNull();
  cleanup();

  const googleLogin = vi.fn();
  const withoutProvider = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false, google: false }),
    googleLogin,
  });
  render(<AccountPage client={withoutProvider} />);
  expect(await screen.findByRole("button", { name: "邮箱登录" })).not.toBeNull();
  expect(screen.queryByRole("button", { name: "使用 Google 登录" })).toBeNull();
  expect(googleLogin).not.toHaveBeenCalled();
});

test("a cancelled Google sign-in stays silent", async () => {
  const googleLogin = vi.fn().mockRejectedValue({ code: "account_cancelled" });
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: false, phone: false, google: true }),
    googleLogin,
  });
  render(<AccountPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Google 登录" }));
  await waitFor(() => expect(googleLogin).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "使用 Google 登录" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  expect(screen.queryByRole("alert")).toBeNull();
});

test("a pending Google sign-in can be abandoned without waiting for the browser", async () => {
  let rejectLogin: (reason: unknown) => void = () => undefined;
  const googleLogin = vi.fn(
    () =>
      new Promise<{ user?: null }>((_, reject) => {
        rejectLogin = reject;
      }),
  );
  const googleCancel = vi.fn(async () => rejectLogin({ code: "account_cancelled" }));
  const onCancelLogin = vi.fn();
  const client = account({
    providers: vi.fn().mockResolvedValue({ email: true, phone: false, google: true }),
    googleLogin,
    googleCancel,
  });
  render(<AccountPage client={client} onCancelLogin={onCancelLogin} />);
  fireEvent.click(await screen.findByRole("button", { name: "使用 Google 登录" }));
  const cancel = await screen.findByRole("button", { name: "取消 Google 登录" });
  expect(
    (screen.getByRole("button", { name: "正在等待浏览器完成 Google 登录…" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  expect((screen.getByRole("button", { name: "取消" }) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(cancel);
  expect(googleCancel).toHaveBeenCalledTimes(1);
  await waitFor(() =>
    expect(
      (screen.getByRole("button", { name: "使用 Google 登录" }) as HTMLButtonElement).disabled,
    ).toBe(false),
  );
  expect(screen.queryByRole("button", { name: "取消 Google 登录" })).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "使用 Google 登录" }));
  await screen.findByRole("button", { name: "取消 Google 登录" });
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(googleCancel).toHaveBeenCalledTimes(2);
  expect(onCancelLogin).toHaveBeenCalledTimes(1);
});

test("mobile profile card opens a back-stack page with account actions", async () => {
  window.history.replaceState({ msimeSettings: true, page: "account" }, "");
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="android" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  expect(await screen.findByRole("heading", { name: "编辑资料" })).not.toBeNull();
  expect(screen.getByRole("button", { name: "退出登录" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "‹ 返回" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "编辑个人资料" })).not.toBeNull());
});
test("account deletion requires its destructive confirmation", async () => {
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} />);
  await screen.findByRole("button", { name: "编辑个人资料" });
  fireEvent.click(screen.getByRole("button", { name: "注销账号" }));
  expect(client.deleteAccount).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认注销账号" }));
  await waitFor(() => expect(client.deleteAccount).toHaveBeenCalledTimes(1));
});

test("account errors are stable and never expose backend text", async () => {
  const client = account({
    providers: vi
      .fn()
      .mockRejectedValue({ code: "account_rate_limited", message: "private backend detail" }),
  });
  render(<AccountPage client={client} />);
  expect((await screen.findByRole("alert")).textContent).toBe("操作过于频繁，请稍后再试。");
  expect(screen.queryByText(/private backend detail/)).toBeNull();
});

test("account cancellation does not show a stale error alert", async () => {
  const client = account({
    status: vi.fn().mockRejectedValue({ code: "account_cancelled" }),
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByRole("heading", { name: "欢迎来到水杉" })).not.toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("an aborted profile request leaves the account page quiet", async () => {
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    profile: vi
      .fn()
      .mockRejectedValue(new DOMException("The user aborted a request.", "AbortError")),
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByRole("heading", { name: "账号" })).not.toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("settings sync cancellation does not become a visible account error", async () => {
  const cancelled = vi.fn().mockRejectedValue({ code: "account_cancelled" });
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    settingsSync: {
      schema: cancelled,
      load: cancelled,
      upload: vi.fn(),
      apply: vi.fn(),
    },
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByRole("heading", { name: "设置同步" })).not.toBeNull();
  await waitFor(() => expect(screen.queryByText("操作已取消，请重试。")).toBeNull());
  expect(screen.queryByRole("alert")).toBeNull();
});

test("ignores a same-tick duplicate cloud settings refresh", async () => {
  const schema = {
    fields: { "input.schema": { type: "string" } },
    maximumBytes: 65536,
    updateMode: "replace" as const,
    revisionRequired: true,
  };
  const load = vi.fn().mockResolvedValue({ revision: 7, settings: { "input.schema": "quanpin" } });
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    settingsSync: {
      schema: vi.fn().mockResolvedValue(schema),
      load,
      upload: vi.fn(),
      apply: vi.fn(),
    },
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByText("云端版本：7")).not.toBeNull();
  const refresh = screen.getByRole("button", { name: "刷新云端设置" });

  act(() => {
    fireEvent.click(refresh);
    fireEvent.click(refresh);
  });
  expect(load).toHaveBeenCalledTimes(2);
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
});

test("logged-in accounts can open their published skin list", async () => {
  const openPublishedSkins = vi.fn();
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} onOpenPublishedSkins={openPublishedSkins} />);
  fireEvent.click(await screen.findByRole("button", { name: "我发布的皮肤" }));
  expect(openPublishedSkins).toHaveBeenCalledTimes(1);
});

test("logged-in accounts expose local designs and every community collection", async () => {
  const openLocalDesigns = vi.fn();
  const openCommunity = vi.fn();
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(
    <AccountPage
      client={client}
      onOpenLocalDesigns={openLocalDesigns}
      onOpenCommunity={openCommunity}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "打开设计器" }));
  fireEvent.click(screen.getByRole("button", { name: "我发布的皮肤" }));
  fireEvent.click(screen.getByRole("button", { name: "我发布的词库" }));
  fireEvent.click(screen.getByRole("button", { name: "我发布的回复模板" }));
  fireEvent.click(screen.getByRole("button", { name: "收藏的词库" }));
  fireEvent.click(screen.getByRole("button", { name: "收藏的回复模板" }));
  expect(openLocalDesigns).toHaveBeenCalledTimes(1);
  expect(openCommunity.mock.calls).toEqual([
    ["published-skins"],
    ["published-dictionary"],
    ["published-reply"],
    ["saved-dictionary"],
    ["saved-reply"],
  ]);
});

test("logged-in mobile accounts expose direct cloud dictionary and clipboard entries", async () => {
  const openCloudDictionary = vi.fn();
  const openCloudClipboard = vi.fn();
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(
    <AccountPage
      client={client}
      onOpenCloudDictionary={openCloudDictionary}
      onOpenCloudClipboard={openCloudClipboard}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "云词库" }));
  fireEvent.click(screen.getByRole("button", { name: "云剪贴板" }));
  expect(openCloudDictionary).toHaveBeenCalledTimes(1);
  expect(openCloudClipboard).toHaveBeenCalledTimes(1);
});

test("mobile accounts expose about and desktop download actions while signed out", async () => {
  const openAbout = vi.fn();
  const openDesktopDownload = vi.fn();
  render(
    <AccountPage
      client={account()}
      onOpenAbout={openAbout}
      onOpenDesktopDownload={openDesktopDownload}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "关于水杉" }));
  fireEvent.click(screen.getByRole("button", { name: "电脑版下载" }));
  expect(openAbout).toHaveBeenCalledTimes(1);
  expect(openDesktopDownload).toHaveBeenCalledTimes(1);
});

test("mobile accounts can replay the onboarding flow without account state", async () => {
  const replayOnboarding = vi.fn();
  render(<AccountPage client={account()} onReplayOnboarding={replayOnboarding} />);
  fireEvent.click(await screen.findByRole("button", { name: "重新查看新手引导" }));
  expect(replayOnboarding).toHaveBeenCalledTimes(1);
});

test("mobile settings return to My after replaying and skipping onboarding", async () => {
  window.history.replaceState({}, "");
  const client = {
    load: async () => preferences,
    save: vi.fn(),
    host: testHost({ platform: "harmony" }),
    home: {},
    account: account(),
  };
  function ReplayHarness() {
    const [replaying, setReplaying] = useState(false);
    if (replaying) {
      return (
        <WelcomeFlowPage
          actions={{
            platform: "harmony",
            prepareResources: vi.fn().mockResolvedValue(undefined),
            openSystemKeyboardSettings: vi.fn().mockResolvedValue(undefined),
            showInputMethodPicker: vi.fn().mockResolvedValue(undefined),
          }}
          onComplete={async () => setReplaying(false)}
          onSkip={async () => setReplaying(false)}
        />
      );
    }
    return <SettingsPage client={client} onReplayOnboarding={() => setReplaying(true)} />;
  }

  render(<ReplayHarness />);
  const mobileTabs = within(screen.getByRole("navigation", { name: "主要功能" }));
  fireEvent.click(mobileTabs.getByRole("button", { name: "我的" }));
  fireEvent.click(await screen.findByRole("button", { name: "新手引导" }));
  fireEvent.click(await screen.findByRole("button", { name: "跳过" }));

  expect(
    within(await screen.findByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "我的",
      current: "page",
    }),
  ).not.toBeNull();
});

test("mobile accounts group published and saved community resources", async () => {
  const openCommunity = vi.fn();
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(<AccountPage client={client} platform="ios" onOpenCommunity={openCommunity} />);
  const content = within(await screen.findByRole("region", { name: "我的内容" }));
  fireEvent.click(content.getByRole("button", { name: "我发布的皮肤" }));
  fireEvent.click(content.getByRole("button", { name: "我发布的词库" }));
  fireEvent.click(content.getByRole("button", { name: "我发布的回复模板" }));
  fireEvent.click(content.getByRole("button", { name: "收藏的词库" }));
  fireEvent.click(content.getByRole("button", { name: "收藏的回复模板" }));
  expect(openCommunity.mock.calls).toEqual([
    ["published-skins"],
    ["published-dictionary"],
    ["published-reply"],
    ["saved-dictionary"],
    ["saved-reply"],
  ]);
});

test("mobile 我的 follows the design's groups: tools, content, then the walkthrough, help and about", async () => {
  const calls: string[] = [];
  const record = (name: string) => () => void calls.push(name);
  const client = account({ status: vi.fn().mockResolvedValue({ user }) });
  render(
    <AccountPage
      client={client}
      mobile
      onOpenCloudClipboard={record("clipboard")}
      onOpenCloudDictionary={record("dictionary")}
      onOpenDesktopDownload={record("download")}
      onOpenLocalDesigns={record("designs")}
      onReplayOnboarding={record("onboarding")}
      onOpenFeedback={record("feedback")}
      onOpenAbout={record("about")}
    />,
  );
  const tools = within(await screen.findByRole("region", { name: "工具" }));
  expect(tools.getAllByRole("button").map((button) => button.textContent)).toEqual([
    "云剪贴板›",
    "云词库›",
    "其他平台下载›",
  ]);
  fireEvent.click(tools.getByRole("button", { name: "云剪贴板" }));
  fireEvent.click(tools.getByRole("button", { name: "云词库" }));
  fireEvent.click(tools.getByRole("button", { name: "其他平台下载" }));
  fireEvent.click(
    within(screen.getByRole("region", { name: "我的内容" })).getByRole("button", {
      name: "我的设计",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "新手引导" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(calls).toEqual([
    "clipboard",
    "dictionary",
    "download",
    "designs",
    "onboarding",
    "feedback",
    "about",
  ]);
  // The desktop sections this replaces are gone, not doubled.
  expect(screen.queryByRole("button", { name: "打开设计器" })).toBeNull();
  expect(screen.queryByRole("button", { name: "重新查看新手引导" })).toBeNull();
});

test("a touch host without an account client still reaches help and about from 我的", () => {
  const openFeedback = vi.fn();
  const openAbout = vi.fn();
  render(<AccountPage platform="android" onOpenFeedback={openFeedback} onOpenAbout={openAbout} />);
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  fireEvent.click(screen.getByRole("button", { name: "关于" }));
  expect(openFeedback).toHaveBeenCalledOnce();
  expect(openAbout).toHaveBeenCalledOnce();
});

test("local designs remain available without an account", async () => {
  const openLocalDesigns = vi.fn();
  render(<AccountPage client={account()} onOpenLocalDesigns={openLocalDesigns} />);
  fireEvent.click(await screen.findByRole("button", { name: "打开设计器" }));
  expect(openLocalDesigns).toHaveBeenCalledTimes(1);
});

test("mobile app icon choices read system state and use an explicit selection", async () => {
  const set = vi.fn().mockResolvedValue({ supported: true, selected: "forest" });
  const client = account({
    appIcon: {
      info: vi.fn().mockResolvedValue({ supported: true, selected: "classic" }),
      set,
    },
  });
  render(<AccountPage client={client} />);
  expect(
    (await screen.findByRole("button", { name: "原版，经典黑白，简洁如初" })).getAttribute(
      "aria-pressed",
    ),
  ).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "杉林，杉叶青绿，沉静自然" }));
  await waitFor(() => expect(set).toHaveBeenCalledWith("forest"));
  expect(
    screen.getByRole("button", { name: "杉林，杉叶青绿，沉静自然" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("app icon selection ignores a same-tick duplicate", async () => {
  const pending = deferred<{ supported: boolean; selected: string }>();
  const set = vi.fn().mockReturnValue(pending.promise);
  render(
    <AccountPage
      client={account({
        appIcon: {
          info: vi.fn().mockResolvedValue({ supported: true, selected: "classic" }),
          set,
        },
      })}
    />,
  );
  await screen.findByRole("button", { name: "原版，经典黑白，简洁如初" });
  const forest = screen.getByRole("button", { name: "杉林，杉叶青绿，沉静自然" });
  act(() => {
    fireEvent.click(forest);
    fireEvent.click(forest);
  });
  expect(set).toHaveBeenCalledOnce();
  pending.resolve({ supported: true, selected: "forest" });
  await waitFor(() => expect(forest.getAttribute("aria-pressed")).toBe("true"));
});

test("app icon errors are ignored only when the reread system state matches", async () => {
  const info = vi
    .fn()
    .mockResolvedValueOnce({ supported: true, selected: "classic" })
    .mockResolvedValueOnce({ supported: true, selected: "forest" });
  const applied = account({
    appIcon: { info, set: vi.fn().mockRejectedValue({ code: "app_icon" }) },
  });
  const result = render(<AccountPage client={applied} />);
  fireEvent.click(await screen.findByRole("button", { name: "杉林，杉叶青绿，沉静自然" }));
  await waitFor(() => expect(info).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole("alert")).toBeNull();
  result.unmount();

  const unchangedInfo = vi
    .fn()
    .mockResolvedValueOnce({ supported: true, selected: "classic" })
    .mockResolvedValueOnce({ supported: true, selected: "classic" });
  render(
    <AccountPage
      client={account({
        appIcon: {
          info: unchangedInfo,
          set: vi.fn().mockRejectedValue({ code: "app_icon" }),
        },
      })}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "杉林，杉叶青绿，沉静自然" }));
  expect((await screen.findByRole("alert")).textContent).toContain("图标未能更换，请稍后重试。");
});

test("a stale app icon response cannot overwrite a replacement client", async () => {
  let resolveOld!: (value: { supported: boolean; selected: string }) => void;
  const oldInfo = vi.fn(
    () =>
      new Promise<{ supported: boolean; selected: string }>((resolve) => {
        resolveOld = resolve;
      }),
  );
  const nextInfo = vi.fn().mockResolvedValue({ supported: true, selected: "forest" });
  const view = render(
    <AccountPage client={account({ appIcon: { info: oldInfo, set: vi.fn() } })} />,
  );
  await waitFor(() => expect(oldInfo).toHaveBeenCalled());
  view.rerender(<AccountPage client={account({ appIcon: { info: nextInfo, set: vi.fn() } })} />);
  expect(
    (await screen.findByRole("button", { name: "杉林，杉叶青绿，沉静自然" })).getAttribute(
      "aria-pressed",
    ),
  ).toBe("true");
  resolveOld({ supported: true, selected: "classic" });
  await Promise.resolve();
  expect(
    screen.getByRole("button", { name: "杉林，杉叶青绿，沉静自然" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("settings sync requires confirmation and preserves a remote conflict error", async () => {
  const upload = vi
    .fn()
    .mockResolvedValue({ revision: 8, settings: { "input.schema": "shuangpin" } });
  const apply = vi.fn().mockRejectedValue({ code: "account_conflict" });
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    settingsSync: {
      schema: vi.fn().mockResolvedValue({
        fields: { "input.schema": { type: "string" } },
        maximumBytes: 65536,
        updateMode: "replace",
        revisionRequired: true,
      }),
      load: vi.fn().mockResolvedValue({ revision: 7, settings: { "input.schema": "quanpin" } }),
      upload,
      apply,
    },
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByText("云端版本：7")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "上传本机设置" }));
  expect(upload).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认上传" }));
  await waitFor(() => expect(upload).toHaveBeenCalledTimes(1));

  fireEvent.click(screen.getByRole("button", { name: "下载并应用云端设置" }));
  fireEvent.click(screen.getByRole("button", { name: "确认应用" }));
  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith("fixture-user-id", {
      revision: 8,
      settings: { "input.schema": "shuangpin" },
    }),
  );
  expect(await screen.findByText("云端设置已被其他设备更新，请刷新后重新确认。")).not.toBeNull();
});

test("ignores a second settings upload while the first is pending", async () => {
  let resolveUpload!: (value: { revision: number; settings: Record<string, string> }) => void;
  const upload = vi.fn(
    () =>
      new Promise<{ revision: number; settings: Record<string, string> }>((resolve) => {
        resolveUpload = resolve;
      }),
  );
  const client = account({
    status: vi.fn().mockResolvedValue({ user }),
    settingsSync: {
      schema: vi.fn().mockResolvedValue({
        fields: { "input.schema": { type: "string" } },
        maximumBytes: 65536,
        updateMode: "replace",
        revisionRequired: true,
      }),
      load: vi.fn().mockResolvedValue({ revision: 7, settings: { "input.schema": "quanpin" } }),
      upload,
      apply: vi.fn(),
    },
  });
  render(<AccountPage client={client} />);
  expect(await screen.findByText("云端版本：7")).not.toBeNull();
  fireEvent.click(await screen.findByRole("button", { name: "上传本机设置" }));
  const confirm = screen.getByRole("button", { name: "确认上传" });
  act(() => {
    fireEvent.click(confirm);
    fireEvent.click(confirm);
  });
  expect(upload).toHaveBeenCalledOnce();
  resolveUpload({ revision: 8, settings: { "input.schema": "quanpin" } });
  await waitFor(() => expect(upload).toHaveBeenCalledOnce());
});

const preferences: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

test("settings expose My only with a personal capability and omit preference actions there", async () => {
  const without = render(
    <SettingsPage client={{ load: async () => preferences, save: vi.fn() }} />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("button", { name: "账号与同步" })).toBeNull();
  without.unmount();

  render(
    <SettingsPage
      client={{ load: async () => preferences, save: vi.fn(), account: account() }}
      initialPage="account"
    />,
  );
  expect(await screen.findByRole("heading", { name: "账号与同步" })).not.toBeNull();
  await screen.findByText("欢迎来到水杉");
  expect(screen.queryByRole("form", { name: "设置" })).toBeNull();
  expect(screen.queryByRole("button", { name: "重新读取" })).toBeNull();
});

test("iOS exposes My and alternate icons without a fake account client", async () => {
  const appIcon = {
    info: vi.fn().mockResolvedValue({ supported: true, selected: "sky" }),
    set: vi.fn(),
  };
  render(
    <SettingsPage
      client={{
        load: async () => preferences,
        save: vi.fn(),
        host: testHost({ platform: "ios" }),
        appIcon,
      }}
      initialPage="account"
    />,
  );
  expect(await screen.findByRole("heading", { name: "我的" })).not.toBeNull();
  expect(
    (await screen.findByRole("heading", { name: "App 图标" })).closest("section")?.textContent,
  ).toContain("iOS 会使用系统备用图标接口保存选择。");
  expect(
    screen.getByRole("button", { name: "晴空，清透蓝调，轻盈明亮" }).getAttribute("aria-pressed"),
  ).toBe("true");
  expect(screen.queryByText("欢迎来到水杉")).toBeNull();
});

test("macOS settings offer no setup guide to replay; the status notice covers the input source", async () => {
  const replay = vi.fn();
  render(
    <SettingsPage
      client={{
        load: async () => preferences,
        save: vi.fn(),
        account: account(),
        host: testHost({ platform: "macos" }),
        inputSourceStartup: {
          status: vi.fn().mockResolvedValue(null),
          openSettings: vi.fn().mockResolvedValue(undefined),
        },
      }}
      initialPage="account"
      onReplayOnboarding={replay}
    />,
  );
  await screen.findByText("欢迎来到水杉");
  expect(screen.queryByRole("button", { name: "重新查看新手引导" })).toBeNull();
});

const avatarImage = "data:image/png;base64,iVBORw0KGgo=";

test("a signed-in card shows the avatar the host fetched and the Google email", async () => {
  const signedIn: AccountUser = {
    ...user,
    email: "person@example.test",
    avatarUrl: "https://lh3.googleusercontent.com/a/card",
  };
  const avatar = vi.fn().mockResolvedValue(avatarImage);
  render(
    <AccountPage
      client={account({
        status: vi.fn().mockResolvedValue({ user: signedIn }),
        profile: vi.fn().mockResolvedValue({ user: signedIn, providers: ["google"] }),
        avatar,
      })}
    />,
  );
  const card = await screen.findByRole("button", { name: "编辑个人资料" });
  expect(within(card).getByText("person@example.test")).not.toBeNull();
  await waitFor(() => expect(card.querySelector("img")?.getAttribute("src")).toBe(avatarImage));
  // The dialog shows the same avatar without asking the host again.
  fireEvent.click(card);
  const dialog = screen.getByRole("dialog", { name: "编辑个人资料" });
  await waitFor(() => expect(dialog.querySelector("img")?.getAttribute("src")).toBe(avatarImage));
  expect(avatar).toHaveBeenCalledOnce();
  expect(within(dialog).getByText("person@example.test")).not.toBeNull();
});

test("without an avatar, or on a host that does not fetch one, the name's first character stands in", async () => {
  render(
    <AccountPage
      client={account({
        status: vi.fn().mockResolvedValue({
          user: { ...user, avatarUrl: "https://lh3.googleusercontent.com/a/no-loader" },
        }),
      })}
    />,
  );
  const card = await screen.findByRole("button", { name: "编辑个人资料" });
  expect(card.querySelector("img")).toBeNull();
  expect(within(card).getByText("水")).not.toBeNull();
  expect(within(card).getByText("水杉账号已登录")).not.toBeNull();
});

test("the edit dialog uploads and removes a custom avatar without losing the name being typed", async () => {
  const google = "https://lh3.googleusercontent.com/a/dialog";
  const uploaded = "https://media.msime.app/avatars/dialog.jpg";
  const signedIn: AccountUser = { ...user, avatarUrl: google };
  const withUpload: AccountProfile = {
    user: { ...signedIn, avatarUrl: uploaded, avatarUploaded: true },
    providers: ["google"],
  };
  const avatar = vi.fn().mockImplementation(async () => avatarImage);
  const chooseAvatar = vi.fn().mockResolvedValue(withUpload);
  const removeAvatar = vi.fn().mockResolvedValue({ user: signedIn, providers: ["google"] });
  render(
    <AccountPage
      client={account({
        status: vi.fn().mockResolvedValue({ user: signedIn }),
        profile: vi.fn().mockResolvedValue({ user: signedIn, providers: ["google"] }),
        avatar,
        chooseAvatar,
        removeAvatar,
      })}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const dialog = screen.getByRole("dialog", { name: "编辑个人资料" });
  // Only an uploaded avatar can be removed.
  expect(within(dialog).queryByRole("button", { name: "移除头像" })).toBeNull();
  fireEvent.change(within(dialog).getByLabelText("编辑社区昵称"), {
    target: { value: "正在输入的昵称" },
  });
  fireEvent.click(within(dialog).getByRole("button", { name: "更换头像" }));
  expect(await screen.findByText("头像已更新。")).not.toBeNull();
  expect(chooseAvatar).toHaveBeenCalledOnce();
  // The new URL asks the host for the new image.
  await waitFor(() => expect(avatar).toHaveBeenCalledTimes(2));
  expect((within(dialog).getByLabelText("编辑社区昵称") as HTMLInputElement).value).toBe(
    "正在输入的昵称",
  );
  fireEvent.click(within(dialog).getByRole("button", { name: "移除头像" }));
  expect(await screen.findByText("已移除头像。")).not.toBeNull();
  expect(removeAvatar).toHaveBeenCalledOnce();
  expect(within(dialog).queryByRole("button", { name: "移除头像" })).toBeNull();
});

test("closing the file dialog changes nothing, and a file the host refuses says what to pick", async () => {
  const chooseAvatar = vi
    .fn()
    .mockResolvedValueOnce(null)
    .mockRejectedValueOnce({ code: "account_invalid" });
  render(
    <AccountPage client={account({ status: vi.fn().mockResolvedValue({ user }), chooseAvatar })} />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const dialog = screen.getByRole("dialog", { name: "编辑个人资料" });
  fireEvent.click(within(dialog).getByRole("button", { name: "更换头像" }));
  await waitFor(() => expect(chooseAvatar).toHaveBeenCalledOnce());
  expect(screen.queryByText("头像已更新。")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
  fireEvent.click(within(dialog).getByRole("button", { name: "更换头像" }));
  expect((await screen.findByRole("alert")).textContent).toBe(
    "请选择 1 MiB 以内的 PNG 或 JPEG 图片。",
  );
});

test("a host without avatar upload offers no avatar button", async () => {
  render(<AccountPage client={account({ status: vi.fn().mockResolvedValue({ user }) })} />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  const dialog = screen.getByRole("dialog", { name: "编辑个人资料" });
  expect(within(dialog).queryByRole("button", { name: "更换头像" })).toBeNull();
  expect(within(dialog).queryByText(/点头像可更换/)).toBeNull();
});
