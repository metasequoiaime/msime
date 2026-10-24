// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage, ToastProvider, type AccountClient } from "@msime/ui";
import { presentAccountName } from "../support/account-name";

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "");
});

const user = {
  id: "synthetic-provider-user",
  displayName: "合成账号",
  preferredName: "合成账号",
  createdAt: "2026-01-01T00:00:00Z",
};
const profile = {
  user,
  providers: ["email", "sms", "synthetic-provider"],
  providerNames: {
    email: "合成邮件方式",
    sms: "合成短信方式",
    "synthetic-provider": "合成新方式",
  },
};

function client(): AccountClient {
  return {
    status: async () => ({ user }),
    providers: async () => ({ email: true, phone: false }),
    profile: async () => profile,
    requestCode: vi.fn(),
    login: vi.fn(),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
  };
}

test.each([undefined, "ios", "android"] as const)(
  "%s 资料按共享名称和原有顺序显示登录方式",
  async (platform) => {
    render(
      <ToastProvider>
        <AccountPage client={client()} platform={platform} />
      </ToastProvider>,
    );
    fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
    expect(await screen.findByText("合成邮件方式、合成短信方式、合成新方式")).not.toBeNull();
  },
);

test("Harmony 的登录标记和登录方式行直接使用共享名称", async () => {
  render(
    <ToastProvider>
      <AccountPage client={client()} platform="harmony" />
    </ToastProvider>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "编辑个人资料" }));
  expect(await screen.findByText("通过 合成邮件方式 登录")).not.toBeNull();
  expect(screen.getByText("合成邮件方式")).not.toBeNull();
  expect(screen.getByText("合成短信方式")).not.toBeNull();
  expect(screen.getByText("合成新方式")).not.toBeNull();
});
