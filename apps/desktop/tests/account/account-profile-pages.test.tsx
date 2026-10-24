// @vitest-environment jsdom
import { accountProviderNames } from "../support/account-provider-names";
import { accountNameResult, presentAccountName } from "../support/account-name";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  HarmonyProfilePage,
  MobileAccountProfilePage,
} from "../../../../packages/ui/src/account/account-profile-pages";
import type {
  AccountClient,
  AccountProfile,
} from "../../../../packages/ui/src/account/account-model";
import { ACCOUNT_DELETED_VOICE_SAMPLES_KEPT } from "../../../../packages/ui/src/account/use-account-session-actions";

const profile: AccountProfile = {
  user: {
    id: "synthetic-profile-user",
    displayName: "合成昵称",
    preferredName: "合成昵称",
    createdAt: "2026-01-01T00:00:00Z",
  },
  providers: ["email"],
  providerNames: accountProviderNames,
};

function client(overrides: Partial<AccountClient> = {}): AccountClient {
  return {
    status: vi.fn(),
    providers: vi.fn(),
    requestCode: vi.fn(),
    login: vi.fn(),
    profile: vi.fn(),
    namePresentation: presentAccountName,
    rename: vi.fn().mockResolvedValue(profile),
    logout: vi.fn().mockResolvedValue(undefined),
    deleteAccount: vi.fn().mockResolvedValue(undefined),
    clearExpired: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

const variants = ["mobile", "harmony"] as const;

function page(
  variant: (typeof variants)[number],
  accountClient: AccountClient,
  onProfileUpdated = vi.fn(),
  onSignedOut = vi.fn(),
  onBack = vi.fn(),
) {
  const Page = variant === "mobile" ? MobileAccountProfilePage : HarmonyProfilePage;
  return (
    <Page
      client={accountClient}
      user={profile.user}
      profile={profile}
      onBack={onBack}
      onProfileUpdated={onProfileUpdated}
      onSignedOut={onSignedOut}
    />
  );
}

function editName(variant: (typeof variants)[number], name: string) {
  if (variant === "harmony")
    fireEvent.click(screen.getByRole("button", { name: "昵称，合成昵称" }));
  fireEvent.change(screen.getByRole("textbox", { name: "编辑社区昵称" }), {
    target: { value: name },
  });
}

afterEach(cleanup);

test.each(variants)("%s 资料页独立保存昵称并通知调用方，返回由调用方处理", async (variant) => {
  const updated = {
    ...profile,
    user: { ...profile.user, displayName: "更新合成昵称", preferredName: "更新合成昵称" },
  };
  const accountClient = client({
    namePresentation: async (name) =>
      accountNameResult(name === "  更新合成昵称  " ? "更新合成昵称" : name),
    rename: vi.fn().mockResolvedValue(updated),
  });
  const onProfileUpdated = vi.fn();
  const onBack = vi.fn();
  const historyState = window.history.state;
  render(page(variant, accountClient, onProfileUpdated, vi.fn(), onBack));
  editName(variant, "  更新合成昵称  ");
  const save = screen.getByRole("button", { name: variant === "mobile" ? "保存昵称" : "保存" });
  await waitFor(() => expect(save).toHaveProperty("disabled", false));
  fireEvent.click(save);
  await waitFor(() => expect(onProfileUpdated).toHaveBeenCalledWith(updated));
  expect(accountClient.rename).toHaveBeenCalledWith("更新合成昵称");
  if (variant === "harmony") expect(screen.queryByRole("dialog", { name: "昵称" })).toBeNull();
  else expect(screen.getByRole("status").textContent).toBe("昵称已更新。");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: /返回/ })).toHaveProperty("disabled", false),
  );
  fireEvent.click(screen.getByRole("button", { name: /返回/ }));
  expect(onBack).toHaveBeenCalledOnce();
  expect(window.history.state).toBe(historyState);
  expect(accountClient.status).not.toHaveBeenCalled();
  expect(accountClient.profile).not.toHaveBeenCalled();
  expect(accountClient.providers).not.toHaveBeenCalled();
});

test.each(variants)("%s 资料页保留注销确认和对应退出回调参数", async (variant) => {
  const accountClient = client({
    deleteAccount: vi.fn().mockResolvedValue({ voice_samples: { ok: false } }),
  });
  const onSignedOut = vi.fn();
  render(page(variant, accountClient, vi.fn(), onSignedOut));
  fireEvent.click(screen.getByRole("button", { name: "注销账号" }));
  fireEvent.click(within(screen.getByRole("alertdialog")).getByRole("button", { name: "取消" }));
  expect(accountClient.deleteAccount).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "注销账号" }));
  fireEvent.click(
    within(screen.getByRole("alertdialog")).getByRole("button", {
      name: variant === "mobile" ? "确认" : "确认注销账号",
    }),
  );
  await waitFor(() => expect(onSignedOut).toHaveBeenCalledOnce());
  expect(onSignedOut.mock.calls).toEqual(
    variant === "mobile" ? [[]] : [[ACCOUNT_DELETED_VOICE_SAMPLES_KEPT]],
  );
  expect(accountClient.deleteAccount).toHaveBeenCalledOnce();
});

test.each(variants)("%s 独立资料页忽略被替换客户端的迟到昵称结果", async (variant) => {
  let finish!: (value: AccountProfile) => void;
  const old = client({
    rename: vi.fn(
      () =>
        new Promise<AccountProfile>((resolve) => {
          finish = resolve;
        }),
    ),
  });
  const onProfileUpdated = vi.fn();
  const view = render(page(variant, old, onProfileUpdated));
  editName(variant, "旧请求合成昵称");
  const save = screen.getByRole("button", { name: variant === "mobile" ? "保存昵称" : "保存" });
  await waitFor(() => expect(save).toHaveProperty("disabled", false));
  fireEvent.click(save);
  await waitFor(() => expect(old.rename).toHaveBeenCalledOnce());
  view.rerender(page(variant, client(), onProfileUpdated));
  await act(async () =>
    finish({
      ...profile,
      user: { ...profile.user, displayName: "迟到合成昵称", preferredName: "迟到合成昵称" },
    }),
  );
  expect(onProfileUpdated).not.toHaveBeenCalled();
  expect(screen.queryByText("迟到合成昵称")).toBeNull();
  expect(screen.queryByText("昵称已更新。")).toBeNull();
});
