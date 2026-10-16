// @vitest-environment jsdom
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  type AccountClient,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunitySkinClient,
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
    global_theme: "shuishan",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const user = {
  id: "fixture-user-id",
  displayName: "水杉测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};

function account(): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user }),
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    requestCode: vi.fn(),
    login: vi.fn(),
    profile: vi.fn().mockResolvedValue({ user, providers: ["email"] }),
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
  };
}

function reply(id: string, name: string): CommunityResource {
  return {
    id,
    kind: "reply",
    name,
    description: "合成的说明",
    author: "示例作者",
    content: { prompt: "请简洁、礼貌地回复。" },
    revision: 1,
    saves: 2,
    saved: true,
    owned: false,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  };
}

function skinsClient(): CommunitySkinClient {
  return {
    list: vi.fn().mockResolvedValue({ skins: [], has_more: false }),
    detail: vi.fn(),
    download: vi.fn(),
    rate: vi.fn(),
    publish: vi.fn(),
    unpublish: vi.fn(),
    setCategory: vi.fn(),
    finishTrial: vi.fn(),
  };
}

function resourcesClient(
  items = [reply("10000000-0000-4000-8000-000000000101", "礼貌回复")],
): CommunityResourceClient {
  return {
    list: vi.fn().mockResolvedValue({ items, has_more: false }),
    detail: vi.fn(async (id: string) => items.find((item) => item.id === id)!),
    publish: vi.fn(),
    apply: vi.fn(),
    save: vi.fn().mockResolvedValue(undefined),
    rate: vi.fn(),
    unpublish: vi.fn(),
    storeReply: vi.fn().mockResolvedValue(undefined),
    removeReply: vi.fn(),
  };
}

function renderPhone(
  initialPage: string,
  { platform = "harmony", resources = resourcesClient() } = {},
) {
  const skins = skinsClient();
  render(
    <SettingsPage
      initialPage={initialPage}
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockResolvedValue(initial),
        host: testHost({ platform }),
        home: {},
        account: account(),
        communitySkins: skins,
        communityResources: resources,
        customSkinLibrary: { load: vi.fn().mockResolvedValue([]), mutate: vi.fn() },
      }}
    />,
  );
  return { skins, resources };
}

const communityRow = async (name: string) =>
  within(await screen.findByRole("region", { name: "社区作品" })).findByRole("button", { name });

// 「我的 → 收藏的回复模板」不再去「社区」标签页：它打开「AI 辅助」页，并在上面打开收藏范围的回复模板图库。返回依次关掉详情、图库，再回到「我的」。
test("a 我的 community row opens the gallery on its feature page, and back unwinds detail, gallery and page", async () => {
  const { resources } = renderPhone("account");
  fireEvent.click(await communityRow("收藏的回复模板"));

  expect(await screen.findByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();
  const gallery = await screen.findByRole("dialog", { name: "社区回复模板" });
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("reply", "saved", "", 0));
  expect(window.history.state).toMatchObject({
    page: "ai",
    communityGallery: { category: "reply", scope: "saved", initialMine: false },
  });
  // 底部标签栏里没有「社区」。
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).queryByRole("button", { name: "社区" })).toBeNull();

  fireEvent.click(await within(gallery).findByRole("button", { name: "查看回复模板 礼貌回复" }));
  expect(await screen.findByRole("heading", { name: "礼貌回复" })).toBeTruthy();
  expect(window.history.state).toMatchObject({
    page: "ai",
    communityDetail: { kind: "reply", id: "10000000-0000-4000-8000-000000000101" },
  });

  window.history.back();
  expect(
    await within(screen.getByRole("dialog", { name: "社区回复模板" })).findByRole("button", {
      name: "查看回复模板 礼貌回复",
    }),
  ).toBeTruthy();

  window.history.back();
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "社区回复模板" })).toBeNull());
  expect(screen.getByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();

  window.history.back();
  expect(await screen.findByRole("heading", { level: 1, name: "我的" })).toBeTruthy();
});

test("我发布的皮肤 opens the skin gallery on 皮肤 with only the user's own skins", async () => {
  const { skins } = renderPhone("account");
  fireEvent.click(await communityRow("我发布的皮肤"));

  expect(await screen.findByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();
  const gallery = await screen.findByRole("dialog", { name: "社区皮肤" });
  await waitFor(() => expect(skins.list).toHaveBeenCalledWith(0, "", true, null));
  expect(
    within(gallery).getByRole("button", { name: "我的作品" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("我发布的词库 opens the dictionary gallery on 词库 in the user's own scope", async () => {
  const { resources } = renderPhone("account");
  fireEvent.click(await communityRow("我发布的词库"));

  expect(await screen.findByRole("heading", { level: 1, name: "词库" })).toBeTruthy();
  expect(await screen.findByRole("dialog", { name: "社区词库" })).toBeTruthy();
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "mine", "", 0));
});

// 鸿蒙手机皮肤页的「社区」分类直接就是完整的社区皮肤图库（搜索、分类、详情、发布），不再是网格里的第一页预览加「浏览更多皮肤」，与 Android 皮肤页一致。
test("the HarmonyOS phone's 社区 tab is the full skin gallery on the page itself", async () => {
  const { skins } = renderPhone("skin");
  const tabs = await screen.findByRole("tablist", { name: "皮肤分类" });
  expect(screen.queryByRole("button", { name: "浏览更多皮肤" })).toBeNull();
  expect(skins.list).not.toHaveBeenCalled();

  fireEvent.click(within(tabs).getByRole("tab", { name: "社区" }));
  const panel = screen.getByRole("tabpanel", { name: "社区" });
  expect(within(panel).getByRole("search")).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "发布设计" })).toBeTruthy();
  await waitFor(() => expect(skins.list).toHaveBeenCalledWith(0, "", false, null));
  // 图库自带搜索和发布表单，嵌在设置的 <form> 里会被 WebView 当成外层表单提交，所以和桌面的「社区皮肤」一样画在表单之外；它在页面里，不是盖在页面上的子视图。
  expect(panel.closest("form")).toBeNull();
  expect(screen.queryByRole("dialog", { name: "社区皮肤" })).toBeNull();
  expect(screen.queryByRole("group", { name: "皮肤" })).toBeNull();
  expect(screen.getByRole("heading", { level: 1, name: "皮肤" })).toBeTruthy();

  fireEvent.click(within(tabs).getByRole("tab", { name: "内置" }));
  expect(screen.queryByRole("tabpanel", { name: "社区" })).toBeNull();
  expect(
    within(screen.getByRole("group", { name: "皮肤" })).getByRole("button", { name: "跟随系统" }),
  ).toBeTruthy();
});

// 图库自带搜索和发布表单，不能嵌在设置的 <form> 里：嵌套的表单在 WebView 里会被当成外层表单提交。
test("the gallery is drawn outside the settings form", async () => {
  renderPhone("account");
  fireEvent.click(await communityRow("我发布的皮肤"));
  const gallery = await screen.findByRole("dialog", { name: "社区皮肤" });
  expect(gallery.closest("form")).toBeNull();
});

const replies = Array.from({ length: 6 }, (_, index) =>
  reply(`10000000-0000-4000-8000-00000000020${index}`, `回复模板${index + 1}`),
);

// 鸿蒙「社区」标签页里叫「短语」的那一段是 AI 高情商回复用的回复模板，现在列在「AI 辅助」页上：前几个可以直接添加，最后一行打开完整图库。
test("the AI page lists the first community reply templates and adds one to the reply keyboard", async () => {
  const resources = resourcesClient(replies);
  renderPhone("ai", { resources });

  const group = await screen.findByRole("region", { name: "回复模板" });
  await within(group).findByRole("button", { name: "添加回复模板 回复模板1" });
  expect(within(group).queryByRole("button", { name: "添加回复模板 回复模板6" })).toBeNull();
  expect(resources.list).toHaveBeenCalledWith("reply", "", "", 0);

  const add = within(group).getByRole("button", { name: "添加回复模板 回复模板2" });
  fireEvent.click(add);
  await waitFor(() => expect(add.textContent).toBe("已添加"));
  expect(resources.save).toHaveBeenCalledWith(replies[1].id, true);
  expect(resources.detail).toHaveBeenCalledWith(replies[1].id);
  expect(resources.storeReply).toHaveBeenCalledWith(replies[1]);
  expect(await screen.findByText("已添加「回复模板2」到高情商回复键盘")).toBeTruthy();

  fireEvent.click(within(group).getByRole("button", { name: "浏览更多回复模板" }));
  const gallery = await screen.findByRole("dialog", { name: "社区回复模板" });
  expect(within(gallery).getByRole("button", { name: "全部" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  expect(window.history.state).toMatchObject({
    page: "ai",
    communityGallery: { category: "reply", scope: "", initialMine: false },
  });
});

// 其他触屏宿主没有 toast，「添加」的结果写在组里。
test("another touch host shows the reply templates and says what 添加 did in the group", async () => {
  const resources = resourcesClient(replies);
  renderPhone("ai", { platform: "android", resources });

  const group = await screen.findByRole("region", { name: "回复模板" });
  fireEvent.click(await within(group).findByRole("button", { name: "添加回复模板 回复模板1" }));
  expect((await within(group).findByRole("status")).textContent).toBe(
    "已添加「回复模板1」到高情商回复键盘",
  );
});

test("the desktop AI page has no reply template group", async () => {
  const resources = resourcesClient(replies);
  renderPhone("ai", { platform: "windows", resources });

  expect(await screen.findByRole("heading", { level: 1, name: "AI 辅助" })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "回复模板" })).toBeNull();
  expect(resources.list).not.toHaveBeenCalled();
});

// 其他触屏宿主的词库页是桌面表单，社区词库从「社区词库」一组进入，图库同样在本页上打开。
test("another touch host opens the dictionary gallery from the 词库 page", async () => {
  const { resources } = renderPhone("dictionary", { platform: "android" });

  fireEvent.click(await screen.findByRole("button", { name: "浏览更多词库" }));
  expect(await screen.findByRole("dialog", { name: "社区词库" })).toBeTruthy();
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "", "", 0));
  expect(screen.getByRole("heading", { level: 1, name: "词库" })).toBeTruthy();
});
