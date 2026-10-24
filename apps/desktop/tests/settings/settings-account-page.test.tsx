// @vitest-environment jsdom
import { presentAccountName } from "../support/account-name";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsAccountPage, type AccountClient, type SettingsAccountPageProps } from "@msime/ui";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

function account(): AccountClient {
  const user = {
    id: "fixture-account",
    displayName: "合成测试账号",
    preferredName: "合成测试账号",
    createdAt: "2026-01-01T00:00:00Z",
  };
  return {
    status: vi.fn().mockResolvedValue({ user }),
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    requestCode: vi.fn(),
    login: vi.fn(),
    profile: vi.fn().mockResolvedValue({ user, providers: [] }),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
  };
}

function model(): SettingsAccountPageProps["model"] {
  return {
    client: { load: vi.fn(), save: vi.fn(), account: account() },
    draft: undefined,
    androidPlatform: false,
    iosPlatform: true,
    harmonyPlatform: false,
    mobilePlatform: true,
    macosPlatform: false,
    accountLoginReturnPage: null,
    finishAccountLogin: vi.fn(),
    openLocalDesigns: vi.fn(),
    openCommunity: vi.fn(),
    selectPage: vi.fn(),
    setDictionaryTab: vi.fn(),
    setError: vi.fn(),
    availablePages: [],
    openExternalUrl: vi.fn().mockResolvedValue(undefined),
    currentAppVersion: "0.0.0",
  };
}

test("账号接线保留宿主方法接收者并向外壳报告云端失败", async () => {
  const state = model();
  state.client.openCloudDictionary = vi.fn(function (this: unknown) {
    expect(this).toBe(state.client);
    return Promise.reject(new Error("synthetic"));
  });
  state.client.openCloudClipboard = vi.fn(function (this: unknown) {
    expect(this).toBe(state.client);
    return Promise.reject(new Error("synthetic"));
  });
  render(<SettingsAccountPage model={state} />);
  fireEvent.click(await screen.findByRole("button", { name: "云词库" }));
  fireEvent.click(screen.getByRole("button", { name: "云剪贴板" }));
  await waitFor(() => {
    expect(state.setError).toHaveBeenCalledWith("无法打开云词库，请重试。");
    expect(state.setError).toHaveBeenCalledWith("无法打开云剪贴板，请重试。");
  });
  expect(state.client.openCloudDictionary).toHaveBeenCalledOnce();
  expect(state.client.openCloudClipboard).toHaveBeenCalledOnce();
});

test("账号反馈入口按触屏能力更新且继续使用外壳导航", async () => {
  const state = model();
  const view = render(<SettingsAccountPage model={state} />);
  await screen.findByRole("button", { name: "编辑个人资料" });
  expect(screen.queryByRole("button", { name: "帮助与反馈" })).toBeNull();
  view.rerender(
    <SettingsAccountPage
      model={{ ...state, availablePages: [{ id: "feedback", title: "帮助与反馈", icon: "" }] }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  expect(state.selectPage).toHaveBeenCalledWith("feedback");
  fireEvent.click(screen.getByRole("button", { name: /^(关于|关于水杉)$/ }));
  expect(state.selectPage).toHaveBeenCalledWith("about");
});

test("macOS 账号接线不暴露原生云端入口或触屏支持动作", async () => {
  const state = model();
  state.macosPlatform = true;
  state.iosPlatform = false;
  state.mobilePlatform = false;
  state.client.openCloudDictionary = vi.fn();
  state.client.openCloudClipboard = vi.fn();
  state.availablePages = [{ id: "feedback", title: "帮助与反馈", icon: "" }];
  const replay = vi.fn();
  render(<SettingsAccountPage model={state} onReplayOnboarding={replay} />);
  await screen.findByRole("button", { name: "编辑个人资料" });
  expect(screen.queryByRole("button", { name: "云词库" })).toBeNull();
  expect(screen.queryByRole("button", { name: "云剪贴板" })).toBeNull();
  expect(screen.queryByRole("button", { name: "帮助与反馈" })).toBeNull();
  expect(screen.queryByRole("button", { name: "新手引导" })).toBeNull();
  expect(replay).not.toHaveBeenCalled();
});
