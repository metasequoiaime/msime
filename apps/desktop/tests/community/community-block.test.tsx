// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import {
  CommunityBlocksContext,
  CommunityPluginsPage,
  CommunityResourcesPage,
  communityBlockDay,
  type CommunityBlockClient,
  type CommunityBlocksValue,
  type CommunityPlugin,
  type CommunityPluginClient,
  type CommunityResource,
  type CommunityResourceClient,
} from "@msime/ui";
import { communityBlockMessage } from "../../../../packages/ui/src/community/community-helpers";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const otherId = "30000000-0000-4000-8000-000000000001";
const ownId = "30000000-0000-4000-8000-000000000002";
const blockId = "40000000-0000-4000-8000-000000000001";

function plugin(id: string, name: string, overrides: Partial<CommunityPlugin> = {}) {
  return {
    id,
    kind: "sound",
    plugin_id: "rain",
    name,
    description: "",
    author: "示例作者",
    version: "1.0.0",
    license: "MIT",
    size: 2048,
    sha256: "0".repeat(64),
    downloads: 0,
    rating_count: 0,
    rating_average: 0,
    owned: false,
    my_rating: 0,
    created_at: "2026-09-30T00:00:00Z",
    ...overrides,
  } satisfies CommunityPlugin;
}

const other = plugin(otherId, "别人的雨声");
const own = plugin(ownId, "我的雨声", { owned: true, author: "我" });

function pluginClient(overrides: Partial<CommunityPluginClient> = {}): CommunityPluginClient {
  return {
    list: vi.fn().mockResolvedValue({ plugins: [other, own], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => (id === otherId ? other : own)),
    packPreview: vi.fn(),
    publish: vi.fn(),
    install: vi.fn(),
    rate: vi.fn().mockResolvedValue({ stars: 5 }),
    delete: vi.fn().mockResolvedValue({ deleted: true }),
    report: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function blockClient(overrides: Partial<CommunityBlockClient> = {}): CommunityBlockClient {
  return {
    block: vi.fn().mockResolvedValue({
      id: blockId,
      name: "示例作者",
      created_at: "2026-10-31T03:00:00Z",
    }),
    list: vi.fn().mockResolvedValue([]),
    unblock: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function blocks(
  client: CommunityBlockClient,
  signedIn = true,
  onLogin = vi.fn(),
): CommunityBlocksValue {
  return { client, signedIn: vi.fn().mockResolvedValue(signedIn), onLogin };
}

function withBlocks(value: CommunityBlocksValue | null, children: ReactNode) {
  return (
    <CommunityBlocksContext.Provider value={value}>{children}</CommunityBlocksContext.Provider>
  );
}

async function openOther() {
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 别人的雨声" }));
  await screen.findByRole("heading", { name: "别人的雨声" });
}

test("the blocked day is the local calendar date, and empty when the time does not parse", () => {
  const instant = "2026-10-31T23:30:00.123456Z";
  const date = new Date(instant);
  const pad = (value: number) => String(value).padStart(2, "0");
  expect(communityBlockDay(instant)).toBe(
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
  );
  expect(communityBlockDay("2026-10-31T09:00:00+09:00")).toMatch(/^2026-10-(30|31)$/);
  expect(communityBlockDay("昨天")).toBe("");
});

test("block failures read the same on every host and a cancelled one says nothing", () => {
  expect(communityBlockMessage({ code: "community_invalid" }, true)).toBe("不能屏蔽自己。");
  expect(communityBlockMessage({ code: "community_conflict" }, true)).toBe(
    "最多屏蔽 500 位作者，请先取消一些。",
  );
  expect(communityBlockMessage({ code: "community_unauthorized" }, true)).toBe(
    "登录水杉账号后才能屏蔽作者。",
  );
  expect(communityBlockMessage({ code: "community_rate_limited" }, true)).toBe(
    "请求过于频繁，请稍后再试。",
  );
  expect(communityBlockMessage({ code: "community_account_banned" }, true)).toBe(
    "该账号已被封禁，暂时无法使用账号相关功能",
  );
  expect(communityBlockMessage({ code: "community_cancelled" }, true)).toBeNull();
  // 取消屏蔽时 `community_invalid` 不是「屏蔽自己」。
  expect(communityBlockMessage({ code: "community_invalid" })).toBe("内容无效，请修改后重试。");
  expect(communityBlockMessage(new Error("boom"), true)).toBe("社区暂时不可用，请稍后重试。");
});

test("a signed-in user confirms, blocks the author, and the detail closes onto a refreshed list", async () => {
  const gallery = pluginClient();
  const client = blockClient();
  render(withBlocks(blocks(client), <CommunityPluginsPage client={gallery} />));
  await openOther();
  expect(gallery.list).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));
  const dialog = await screen.findByRole("alertdialog", { name: "屏蔽作者「示例作者」？" });
  expect(
    within(dialog).getByText(
      "屏蔽后，社区里不再显示这位作者发布的皮肤、插件、词库和模板。可以在「我的 › 隐私 › 已屏蔽的作者」里取消屏蔽。",
    ),
  ).toBeDefined();
  expect(within(dialog).getByRole("button", { name: "取消" })).toBeDefined();
  fireEvent.click(within(dialog).getByRole("button", { name: "屏蔽" }));

  await waitFor(() => expect(client.block).toHaveBeenCalledWith("plugins", otherId));
  expect(await screen.findByText("已屏蔽「示例作者」")).toBeDefined();
  expect(screen.queryByRole("heading", { name: "别人的雨声" })).toBeNull();
  await waitFor(() => expect(gallery.list).toHaveBeenCalledTimes(2));
});

test("cancelling the confirmation blocks nothing", async () => {
  const client = blockClient();
  render(withBlocks(blocks(client), <CommunityPluginsPage client={pluginClient()} />));
  await openOther();
  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));
  const dialog = await screen.findByRole("alertdialog", { name: "屏蔽作者「示例作者」？" });
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("alertdialog", { name: "屏蔽作者「示例作者」？" })).toBeNull();
  expect(client.block).not.toHaveBeenCalled();
  expect(screen.getByRole("heading", { name: "别人的雨声" })).toBeDefined();
});

test("a signed-out user is asked to sign in before anything is confirmed", async () => {
  const client = blockClient();
  const onLogin = vi.fn();
  render(
    withBlocks(blocks(client, false, onLogin), <CommunityPluginsPage client={pluginClient()} />),
  );
  await openOther();
  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));

  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("登录水杉账号后才能屏蔽作者。");
  expect(screen.queryByRole("alertdialog")).toBeNull();
  fireEvent.click(within(alert).getByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledTimes(1);
  expect(client.block).not.toHaveBeenCalled();
});

test.each([
  ["community_invalid", "不能屏蔽自己。", false],
  ["community_conflict", "最多屏蔽 500 位作者，请先取消一些。", false],
  ["community_unauthorized", "登录水杉账号后才能屏蔽作者。", true],
])("a %s refusal keeps the detail open and says why", async (code, message, signIn) => {
  const client = blockClient({ block: vi.fn().mockRejectedValue({ code }) });
  render(withBlocks(blocks(client), <CommunityPluginsPage client={pluginClient()} />));
  await openOther();
  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));
  fireEvent.click(await screen.findByRole("button", { name: "屏蔽" }));

  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain(message);
  expect(within(alert).queryByRole("button", { name: "去登录" }) !== null).toBe(signIn);
  expect(screen.getByRole("heading", { name: "别人的雨声" })).toBeDefined();
});

test("a cancelled block says nothing", async () => {
  const client = blockClient({ block: vi.fn().mockRejectedValue({ code: "community_cancelled" }) });
  render(withBlocks(blocks(client), <CommunityPluginsPage client={pluginClient()} />));
  await openOther();
  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));
  fireEvent.click(await screen.findByRole("button", { name: "屏蔽" }));
  await waitFor(() => expect(client.block).toHaveBeenCalled());
  await waitFor(() => expect(screen.getByRole("button", { name: "屏蔽作者" })).toBeDefined());
  expect(screen.queryByRole("alert")).toBeNull();
});

test("the user's own item and a host without blocking show no 屏蔽作者", async () => {
  render(withBlocks(blocks(blockClient()), <CommunityPluginsPage client={pluginClient()} />));
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 我的雨声" }));
  await screen.findByRole("heading", { name: "我的雨声" });
  expect(screen.queryByRole("button", { name: "屏蔽作者" })).toBeNull();
  cleanup();

  render(withBlocks(null, <CommunityPluginsPage client={pluginClient()} />));
  await openOther();
  expect(screen.getByRole("button", { name: "举报" })).toBeDefined();
  expect(screen.queryByRole("button", { name: "屏蔽作者" })).toBeNull();
});

const dictionary: CommunityResource = {
  id: otherId,
  kind: "dictionary",
  name: "别人的词库",
  description: "",
  author: "词库作者",
  content: { entries: [] },
  revision: 1,
  saves: 0,
  saved: false,
  owned: false,
  rating_count: 0,
  rating_average: 0,
  my_rating: 0,
};

function resourceClient(): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [dictionary], has_more: false }),
    detail: vi.fn().mockResolvedValue(dictionary),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
    report: vi.fn(),
  };
}

// HarmonyOS 在实名认证到位之前不给投稿类功能，举报也在其中；屏蔽不是投稿，照常提供。
test("blocking stays when community contributions are off, and names dictionaries by their kind", async () => {
  const gallery = resourceClient();
  const client = blockClient({
    block: vi.fn().mockResolvedValue({ id: blockId, name: "词库作者", created_at: "" }),
  });
  render(
    withBlocks(
      blocks(client),
      <CommunityResourcesPage client={gallery} kind="dictionary" contributions={false} />,
    ),
  );
  fireEvent.click(await screen.findByRole("button", { name: /别人的词库/ }));
  await screen.findByRole("heading", { name: "别人的词库" });
  expect(screen.queryByRole("button", { name: "举报" })).toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "屏蔽作者" }));
  fireEvent.click(
    within(await screen.findByRole("alertdialog", { name: "屏蔽作者「词库作者」？" })).getByRole(
      "button",
      { name: "屏蔽" },
    ),
  );
  await waitFor(() => expect(client.block).toHaveBeenCalledWith("dictionaries", otherId));
  expect(await screen.findByText("已屏蔽「词库作者」")).toBeDefined();
  await waitFor(() => expect(gallery.list).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole("heading", { name: "别人的词库" })).toBeNull();
});
