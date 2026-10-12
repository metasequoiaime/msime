// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  CommunityPluginsPage,
  CommunityResourcesPage,
  communityReportReasons,
  type CommunityPlugin,
  type CommunityPluginClient,
  type CommunityResource,
  type CommunityResourceClient,
} from "@msime/ui";
import {
  candidateSkinMessage,
  communityPluginMessage,
  communitySkinMessage,
  communitySkinPublishMessage,
  resourceMessage,
} from "../../../../packages/ui/src/community/community-helpers";
import { accountMessage } from "../../../../packages/ui/src/account/account-errors";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const otherId = "30000000-0000-4000-8000-000000000001";
const ownId = "30000000-0000-4000-8000-000000000002";

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
const removed = plugin(ownId, "我的雨声", { owned: true, moderation: "removed" });

function pluginClient(overrides: Partial<CommunityPluginClient> = {}): CommunityPluginClient {
  return {
    list: vi.fn().mockResolvedValue({ plugins: [other, removed], has_more: false }),
    detail: vi.fn().mockImplementation(async (id: string) => (id === otherId ? other : removed)),
    packPreview: vi.fn(),
    publish: vi.fn(),
    install: vi.fn(),
    rate: vi.fn().mockResolvedValue({ stars: 5 }),
    delete: vi.fn().mockResolvedValue({ deleted: true }),
    report: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

test("another user's item offers 举报 with the fixed reasons and files the report", async () => {
  const client = pluginClient();
  render(<CommunityPluginsPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 别人的雨声" }));
  await screen.findByRole("heading", { name: "别人的雨声" });

  fireEvent.click(screen.getByRole("button", { name: "举报" }));
  const form = screen.getByRole("alertdialog", { name: "举报作品" });
  expect(
    within(form)
      .getAllByRole("radio")
      .map((radio) => (radio as HTMLInputElement).value),
  ).toEqual([...communityReportReasons]);
  const submit = within(form).getByRole("button", { name: "提交举报" }) as HTMLButtonElement;
  expect(submit.disabled).toBe(true);
  fireEvent.click(within(form).getByRole("radio", { name: "恶意插件" }));
  fireEvent.change(within(form).getByRole("textbox", { name: "举报补充说明" }), {
    target: { value: "  会删除文件  " },
  });
  fireEvent.click(submit);

  await waitFor(() =>
    expect(client.report).toHaveBeenCalledWith(otherId, "恶意插件", "会删除文件"),
  );
  expect(await screen.findByText("已收到举报，我们会尽快处理。")).toBeDefined();
  expect(screen.queryByRole("alertdialog", { name: "举报作品" })).toBeNull();
});

test("a failed report keeps the form open and shows the reason", async () => {
  const client = pluginClient({
    report: vi.fn().mockRejectedValue({ code: "community_rate_limited" }),
  });
  render(<CommunityPluginsPage client={client} />);
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 别人的雨声" }));
  await screen.findByRole("heading", { name: "别人的雨声" });
  fireEvent.click(screen.getByRole("button", { name: "举报" }));
  fireEvent.click(screen.getByRole("radio", { name: "其他" }));
  fireEvent.click(screen.getByRole("button", { name: "提交举报" }));

  expect(await screen.findByText("请求过于频繁，请稍后再试。")).toBeDefined();
  expect(screen.getByRole("alertdialog", { name: "举报作品" })).toBeDefined();
});

test("the owner's removed item shows 已下架 and no 举报 entry", async () => {
  render(<CommunityPluginsPage client={pluginClient()} />);
  const card = await screen.findByRole("button", { name: "查看插件 我的雨声" });
  expect(card.textContent).toContain("已下架");
  expect(screen.getByRole("button", { name: "查看插件 别人的雨声" }).textContent).not.toContain(
    "已下架",
  );
  fireEvent.click(card);
  await screen.findByRole("heading", { name: "我的雨声" });
  expect(screen.getByText("已下架")).toBeDefined();
  expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
});

test("a host without reporting shows no 举报 entry", async () => {
  render(<CommunityPluginsPage client={pluginClient({ report: undefined })} />);
  fireEvent.click(await screen.findByRole("button", { name: "查看插件 别人的雨声" }));
  await screen.findByRole("heading", { name: "别人的雨声" });
  expect(screen.queryByRole("button", { name: "举报" })).toBeNull();
});

test("the plugin gallery's 我的作品 scope asks the host for the user's own packs", async () => {
  const client = pluginClient();
  render(<CommunityPluginsPage client={client} />);
  await waitFor(() => expect(client.list).toHaveBeenCalledWith(0, "", null, false));
  fireEvent.click(
    within(screen.getByRole("group", { name: "插件范围" })).getByRole("button", {
      name: "我的作品",
    }),
  );
  await waitFor(() => expect(client.list).toHaveBeenLastCalledWith(0, "", null, true));
});

const resource = (overrides: Partial<CommunityResource> = {}): CommunityResource => ({
  id: otherId,
  kind: "reply",
  name: "礼貌回复",
  description: "",
  author: "示例作者",
  content: { prompt: "请礼貌地回复。" },
  revision: 1,
  saves: 0,
  saved: false,
  owned: false,
  rating_count: 0,
  rating_average: 0,
  my_rating: 0,
  ...overrides,
});

function resourceClient(overrides: Partial<CommunityResourceClient> = {}): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items: [resource()], has_more: false }),
    detail: vi.fn().mockResolvedValue(resource()),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn(),
    rate: vi.fn(),
    unpublish: vi.fn(),
    storeReply: vi.fn(),
    removeReply: vi.fn(),
    report: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

test("a shared reply template is reported with its own kind", async () => {
  const client = resourceClient();
  render(<CommunityResourcesPage client={client} kind="reply" />);
  fireEvent.click(await screen.findByRole("button", { name: "查看回复模板 礼貌回复" }));
  await screen.findByRole("heading", { name: "礼貌回复" });
  fireEvent.click(screen.getByRole("button", { name: "举报" }));
  fireEvent.click(screen.getByRole("radio", { name: "垃圾广告" }));
  fireEvent.click(screen.getByRole("button", { name: "提交举报" }));

  await waitFor(() => expect(client.report).toHaveBeenCalledWith("reply", otherId, "垃圾广告", ""));
  expect(await screen.findByText("已收到举报，我们会尽快处理。")).toBeDefined();
});

test("the owner's removed resource shows 已下架", async () => {
  const own = resource({ owned: true, moderation: "removed" });
  render(
    <CommunityResourcesPage
      client={resourceClient({
        list: vi.fn().mockResolvedValue({ items: [own], has_more: false }),
        detail: vi.fn().mockResolvedValue(own),
      })}
      kind="reply"
      initialScope="mine"
    />,
  );
  const card = await screen.findByRole("button", { name: "查看回复模板 礼貌回复" });
  expect(card.textContent).toContain("已下架");
});

test.each([
  ["community_blocked_content", "内容包含不允许发布的词语，请修改后再提交"],
  ["community_screening_unavailable", "审核服务暂时不可用，请稍后重试"],
  ["community_account_banned", "该账号已被封禁，暂时无法使用账号相关功能"],
])("every community page words %s the same way", (code, message) => {
  const error = { code };
  expect(resourceMessage(error)).toBe(message);
  expect(communitySkinMessage(error)).toBe(message);
  expect(communitySkinPublishMessage(error)).toBe(message);
  expect(candidateSkinMessage(error, true)).toBe(message);
  expect(communityPluginMessage(error, true)).toBe(message);
});

test.each([
  [
    "community_unauthorized",
    "请先登录后执行此操作。",
    "登录已失效；仍可退出后匿名浏览。",
    "请先登录后执行此操作。",
  ],
  [
    "community_forbidden",
    "没有权限执行此操作。",
    "没有权限执行此操作；自己的作品不能评分或下架。",
    "下载后才能评分，且不能给自己的作品评分。",
  ],
  ["community_not_found", "作品不存在或已下架。", "作品不存在或已下架。", "作品不存在或已下架。"],
  [
    "community_cancelled",
    "账号状态已变化，请重新加载。",
    "账号状态已变化，请重新加载。",
    "账号状态已变化，请重新加载。",
  ],
  [
    "community_storage",
    "无法安全保存本地模板，请稍后重试。",
    "无法安全读取登录状态，请检查设备安全设置。",
    "无法安全读取登录状态，请检查设备安全设置。",
  ],
])("community status %s keeps page-specific wording", (code, resource, skin, gallery) => {
  const error = { code };
  expect(resourceMessage(error)).toBe(resource);
  expect(communitySkinMessage(error)).toBe(skin);
  expect(candidateSkinMessage(error)).toBe(gallery);
  expect(communityPluginMessage(error)).toBe(gallery);
});

test.each([
  [
    "community_conflict",
    "作品状态已变化或已达到发布上限，请刷新后重试。",
    "发布信息已变更，或皮肤库已达到数量上限。",
    "作品状态已变化，请刷新后重试。",
    "发布信息已变更，或已达到发布上限（最多 20 个、合计 32 MB）。",
  ],
  [
    "community_rate_limited",
    "请求过于频繁，请稍后再试。",
    "发布太频繁，请稍后再试。",
    "请求过于频繁，请稍后再试。",
    "每小时最多发布 10 次，请稍后再试。",
  ],
])(
  "community request status %s keeps gallery and publish wording",
  (code, shared, candidate, plugin, pluginPublish) => {
    const error = { code };
    expect(resourceMessage(error)).toBe(shared);
    expect(communitySkinMessage(error)).toBe(shared);
    expect(candidateSkinMessage(error)).toBe(candidate);
    expect(communityPluginMessage(error)).toBe(plugin);
    expect(communityPluginMessage(error, true)).toBe(pluginPublish);
  },
);

test("publish messages keep their specific wording", () => {
  expect(communitySkinPublishMessage({ code: "community_unauthorized" })).toBe(
    "请先登录后再发布皮肤。",
  );
  expect(candidateSkinMessage({ code: "community_invalid" }, true)).toContain(
    "服务器未接受这款皮肤",
  );
  expect(communityPluginMessage({ code: "community_invalid" }, true)).toContain(
    "服务器未接受这个插件",
  );
});

test("invalid community content keeps gallery and publish wording", () => {
  const error = { code: "community_invalid" };
  expect(resourceMessage(error)).toBe("内容无效，请修改后重试。");
  expect(communitySkinMessage(error)).toBe("搜索内容无效，请修改后重试。");
  expect(candidateSkinMessage(error)).toBe("内容无效，请修改后重试。");
  expect(communityPluginMessage(error)).toBe("内容无效，请修改后重试。");
  expect(candidateSkinMessage(error, true)).toBe(
    "服务器未接受这款皮肤：请确认图片每边不超过 2048 像素、能被正常解码，且皮肤 ID 不与内置主题重名。",
  );
  expect(communityPluginMessage(error, true)).toBe(
    "服务器未接受这个插件：请确认名称、说明和包内容符合发布要求。",
  );
  expect(communitySkinPublishMessage(error)).toBe("名称、说明或皮肤设计不符合发布要求。");
});

test("unknown error codes keep the community fallback", () => {
  expect(resourceMessage({ code: "toString" })).toBe("社区暂时不可用，请稍后重试。");
});

test("a refused upload is never worded as the service being down", () => {
  expect(communityPluginMessage({ code: "community_blocked_content" }, true)).not.toContain(
    "暂时不可用",
  );
});

test.each([
  ["account_blocked_content", "内容包含不允许发布的词语，请修改后再提交"],
  ["account_screening_unavailable", "审核服务暂时不可用，请稍后重试"],
  ["account_banned", "该账号已被封禁，暂时无法使用账号相关功能"],
])("the account page words %s distinctly", (code, message) => {
  expect(accountMessage({ code })).toBe(message);
});
