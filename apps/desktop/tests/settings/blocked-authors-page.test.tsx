// @vitest-environment jsdom
import { testHost } from "../support/host";
import { presentAccountName } from "../support/account-name";
import { settingsFormReady, selectSettingsTab } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  communityBlockDay,
  type AccountClient,
  type CommunityBlockClient,
  type Snapshot,
} from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const initial: Snapshot = {
  format_version: 1,
  revision: 1,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    global_theme: "system",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const first = {
  id: "40000000-0000-4000-8000-000000000001",
  name: "示例作者",
  created_at: "2026-10-30T12:00:00Z",
};
const second = {
  id: "40000000-0000-4000-8000-000000000002",
  name: "另一位作者",
  created_at: "2026-09-01T12:00:00Z",
};

function account(signedIn: boolean): AccountClient {
  return {
    status: vi
      .fn()
      .mockResolvedValue({ user: signedIn ? { id: "user-1", display_name: "我" } : null }),
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    requestCode: vi.fn(),
    login: vi.fn(),
    profile: vi.fn().mockResolvedValue({ id: "user-1", display_name: "我" }),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
  } as unknown as AccountClient;
}

function blocks(overrides: Partial<CommunityBlockClient> = {}): CommunityBlockClient {
  return {
    block: vi.fn(),
    list: vi.fn().mockResolvedValue([first, second]),
    unblock: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

const phone = testHost({ platform: "harmony", mobile_settings: true, panel_windows: false });

function renderPhone(client: { account?: AccountClient; communityBlocks?: CommunityBlockClient }) {
  return render(
    <SettingsPage
      initialPage="privacy"
      client={{ load: async () => initial, save: vi.fn(), host: phone, ...client }}
    />,
  );
}

test("the phone privacy page opens the blocked authors, each with its day and 取消屏蔽", async () => {
  const client = blocks();
  renderPhone({ account: account(true), communityBlocks: client });
  await settingsFormReady();
  const privacy = screen.getByRole("group", { name: "隐私" });
  const row = await within(privacy).findByRole("button", { name: "已屏蔽的作者" });
  await waitFor(() => expect(row.textContent).toContain("社区里不显示这些作者的作品"));
  fireEvent.click(row);

  const page = await screen.findByRole("group", { name: "已屏蔽的作者" });
  expect(await within(page).findByText("示例作者")).toBeDefined();
  expect(within(page).getByText(communityBlockDay(first.created_at))).toBeDefined();
  expect(within(page).getByText("另一位作者")).toBeDefined();
  expect(client.list).toHaveBeenCalledTimes(1);

  fireEvent.click(within(page).getByRole("button", { name: "取消屏蔽 示例作者" }));
  await waitFor(() => expect(client.unblock).toHaveBeenCalledWith(first.id));
  await waitFor(() => expect(within(page).queryByText("示例作者")).toBeNull());
  expect(within(page).getByText("另一位作者")).toBeDefined();

  fireEvent.click(within(page).getByRole("button", { name: "取消屏蔽 另一位作者" }));
  expect(
    await within(page).findByText(
      "没有屏蔽任何作者。在社区作品详情里点「屏蔽作者」，就不会再看到这位作者的作品。",
    ),
  ).toBeDefined();
});

test("signed out, the row says it needs an account and opens sign-in instead of the list", async () => {
  const client = blocks();
  renderPhone({ account: account(false), communityBlocks: client });
  await settingsFormReady();
  const privacy = screen.getByRole("group", { name: "隐私" });
  const row = await within(privacy).findByRole("button", { name: "已屏蔽的作者" });
  await waitFor(() => expect(row.textContent).toContain("登录后可用"));
  fireEvent.click(row);

  await waitFor(() => expect(screen.queryByRole("group", { name: "隐私" })).toBeNull());
  expect(screen.queryByRole("group", { name: "已屏蔽的作者" })).toBeNull();
  expect(client.list).not.toHaveBeenCalled();
});

test("a list that cannot be read says so and reads again on 重试", async () => {
  const list = vi
    .fn()
    .mockRejectedValueOnce({ code: "community_unavailable" })
    .mockResolvedValueOnce([second]);
  render(
    <SettingsPage
      initialPage="blocked-authors"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: phone,
        account: account(true),
        communityBlocks: blocks({ list }),
      }}
    />,
  );
  const page = await screen.findByRole("group", { name: "已屏蔽的作者" });
  expect(await within(page).findByText("暂时无法读取已屏蔽的作者，请重试。")).toBeDefined();
  fireEvent.click(within(page).getByRole("button", { name: "重试" }));
  expect(await within(page).findByText("另一位作者")).toBeDefined();
  expect(list).toHaveBeenCalledTimes(2);
});

test("a host without blocking has no 已屏蔽的作者 row or page", async () => {
  renderPhone({ account: account(true) });
  await settingsFormReady();
  const privacy = screen.getByRole("group", { name: "隐私" });
  expect(within(privacy).queryByRole("button", { name: "已屏蔽的作者" })).toBeNull();
  cleanup();

  render(
    <SettingsPage
      initialPage="blocked-authors"
      client={{ load: async () => initial, save: vi.fn(), host: phone, account: account(true) }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("group", { name: "已屏蔽的作者" })).toBeNull();
});

// 桌面没有「我的 › 隐私」，这一行在「关于」的「许可与隐私」里。
test("desktop hosts list the blocked authors under 关于 › 许可与隐私", async () => {
  const client = blocks();
  render(
    <SettingsPage
      initialPage="about"
      client={{
        load: async () => initial,
        save: vi.fn(),
        host: testHost({ platform: "linux" }),
        account: account(true),
        communityBlocks: client,
      }}
    />,
  );
  await settingsFormReady();
  selectSettingsTab("许可与隐私");
  const about = screen.getByRole("group", { name: "关于" });
  const row = await within(about).findByRole("button", { name: "已屏蔽的作者" });
  await waitFor(() => expect(row.textContent).toContain("社区里不显示这些作者的作品"));
  fireEvent.click(row);
  const page = await screen.findByRole("group", { name: "已屏蔽的作者" });
  expect(await within(page).findByText("示例作者")).toBeDefined();
  fireEvent.click(screen.getByRole("button", { name: "返回关于" }));
  expect(await screen.findByRole("group", { name: "关于" })).toBeDefined();
});
