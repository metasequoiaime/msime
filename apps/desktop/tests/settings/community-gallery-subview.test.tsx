// @vitest-environment jsdom
import { accountProviderNames } from "../support/account-provider-names";
import { presentAccountName } from "../support/account-name";
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import {
  SettingsPage,
  type AccountClient,
  type CommunityResource,
  type CommunityResourceClient,
  type CommunitySkinClient,
  type Snapshot,
} from "@msime/ui";
import { useCommunityGallerySubview } from "../../../../packages/ui/src/settings/use-community-gallery-subview";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

test("desktop callers do not create a mobile gallery history entry", () => {
  window.history.replaceState({ page: "skin" }, "");
  const selectPage = vi.fn();
  const { result } = renderHook(() =>
    useCommunityGallerySubview({
      mobile: false,
      page: "skin",
      selectPage,
      hasSkins: true,
    }),
  );

  act(() => {
    result.current.openCommunityGallery({ category: "skin", scope: "", initialMine: false });
  });

  expect(window.history.state).toEqual({ page: "skin" });
  expect(result.current.communityGallery).toBeNull();
  expect(selectPage).toHaveBeenCalledWith("skin");
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
  preferredName: "水杉测试用户",
  createdAt: "2026-01-01T00:00:00Z",
};

function account(): AccountClient {
  return {
    status: vi.fn().mockResolvedValue({ user }),
    providers: vi.fn().mockResolvedValue({ email: true, phone: false }),
    requestCode: vi.fn(),
    login: vi.fn(),
    profile: vi
      .fn()
      .mockResolvedValue({ user, providers: ["email"], providerNames: accountProviderNames }),
    namePresentation: presentAccountName,
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

// 回复模板从 AI 辅助页进入，收藏筛选在图库里；返回依次关闭详情和图库。
test("回复模板图库保留收藏筛选和逐层返回", async () => {
  const { resources } = renderPhone("ai");
  fireEvent.click(await screen.findByRole("button", { name: "浏览更多回复模板" }));

  expect(await screen.findByRole("heading", { level: 1, name: /^(AI|AI 辅助)$/ })).toBeTruthy();
  const gallery = await screen.findByRole("dialog", { name: "社区回复模板" });
  fireEvent.click(within(gallery).getByRole("button", { name: "收藏" }));
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("reply", "saved", "", 0));
  expect(window.history.state).toMatchObject({
    page: "ai",
    communityGallery: { category: "reply", scope: "", initialMine: false },
  });
  // 底部的「社区」标签没有点亮：回复模板图库盖在「AI 辅助」页上。
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "社区" }).getAttribute("aria-current")).toBeNull();

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
  expect(screen.getByRole("heading", { level: 1, name: /^(AI|AI 辅助)$/ })).toBeTruthy();
});

test("社区皮肤保留我的作品筛选", async () => {
  const { skins } = renderPhone("community");
  const tabs = await screen.findByRole("tablist", { name: "社区分类" });
  const gallery = screen.getByRole("tabpanel", { name: "皮肤" });
  fireEvent.click(within(gallery).getByRole("button", { name: "我的作品" }));
  await waitFor(() => expect(skins.list).toHaveBeenCalledWith(0, "", true, null));
  expect(within(tabs).getByRole("tab", { name: "皮肤" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  expect(
    within(gallery).getByRole("button", { name: "我的作品" }).getAttribute("aria-pressed"),
  ).toBe("true");
});

test("社区词库保留我的作品与收藏筛选", async () => {
  const { resources } = renderPhone("community");
  const tabs = await screen.findByRole("tablist", { name: "社区分类" });
  fireEvent.click(within(tabs).getByRole("tab", { name: "词库" }));
  const gallery = screen.getByRole("tabpanel", { name: "词库" });
  fireEvent.click(within(gallery).getByRole("button", { name: "我的作品" }));
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "mine", "", 0));
  fireEvent.click(within(gallery).getByRole("button", { name: "收藏" }));
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "saved", "", 0));
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
  renderPhone("ai");
  fireEvent.click(await screen.findByRole("button", { name: "浏览更多回复模板" }));
  const gallery = await screen.findByRole("dialog", { name: "社区回复模板" });
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

  expect(await screen.findByRole("heading", { level: 1, name: /^(AI|AI 辅助)$/ })).toBeTruthy();
  expect(screen.queryByRole("region", { name: "回复模板" })).toBeNull();
  expect(resources.list).not.toHaveBeenCalled();
});

// 其他触屏宿主的词库页是桌面表单，社区词库从「社区词库」一组进入：「浏览更多词库」切到底部「社区」标签的「词库」分类，不在词库页上盖一层图库；返回回到词库页。
test("another touch host's 浏览更多词库 switches to the 社区 tab's 词库 category", async () => {
  const { resources } = renderPhone("dictionary", { platform: "android" });

  fireEvent.click(await screen.findByRole("button", { name: "浏览更多词库" }));
  expect(await screen.findByRole("heading", { level: 1, name: "社区" })).toBeTruthy();
  const tabs = screen.getByRole("tablist", { name: "社区分类" });
  expect(within(tabs).getByRole("tab", { name: "词库" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "", "", 0));
  expect(screen.queryByRole("dialog", { name: "社区词库" })).toBeNull();
  expect(window.history.state).toMatchObject({ page: "community", tab: "community" });
  const bar = screen.getByRole("navigation", { name: "主要功能" });
  expect(within(bar).getByRole("button", { name: "社区" }).getAttribute("aria-current")).toBe(
    "page",
  );

  window.history.back();
  expect(await screen.findByRole("heading", { level: 1, name: /^(词库|词库管理)$/ })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "首页" }).getAttribute("aria-current")).toBe(
    "page",
  );
});

// ---- 底部「社区」标签 ----

function dictionaryResource(id: string, name: string): CommunityResource {
  return {
    id,
    kind: "dictionary",
    name,
    description: "合成的词库说明",
    author: "示例作者",
    content: { entries: [{ kind: "pinyin", word: "合成词", code: "he'cheng'ci", weight: 10 }] },
    revision: 1,
    saves: 0,
    saved: false,
    owned: false,
    rating_count: 0,
    rating_average: 0,
    my_rating: 0,
  } as unknown as CommunityResource;
}

// 第二格「社区」的根页：标题「社区」，顶部「皮肤 / 词库」两类，皮肤在前并默认选中，内容是完整的社区皮肤图库（搜索、发布）。它是标签页的根页，没有返回按钮；图库自带表单，画在设置表单之外。
test("the 社区 tab root shows the 皮肤 and 词库 categories with the full skin gallery first", async () => {
  const { skins, resources } = renderPhone("home");
  const bar = await screen.findByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "社区" }));

  expect(await screen.findByRole("heading", { level: 1, name: "社区" })).toBeTruthy();
  expect(within(bar).getByRole("button", { name: "社区" }).getAttribute("aria-current")).toBe(
    "page",
  );
  expect(screen.queryByRole("button", { name: "返回" })).toBeNull();
  const tabs = screen.getByRole("tablist", { name: "社区分类" });
  expect(
    within(tabs)
      .getAllByRole("tab")
      .map((tab) => tab.textContent),
  ).toEqual(["皮肤", "词库"]);
  expect(within(tabs).getByRole("tab", { name: "皮肤" }).getAttribute("aria-selected")).toBe(
    "true",
  );

  const panel = screen.getByRole("tabpanel", { name: "皮肤" });
  expect(within(panel).getByRole("search")).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "发布设计" })).toBeTruthy();
  expect(panel.closest("form")).toBeNull();
  await waitFor(() => expect(skins.list).toHaveBeenCalledWith(0, "", false, null));
  // 没选中的分类不挂载，不提前去拉词库列表。
  expect(resources.list).not.toHaveBeenCalledWith("dictionary", "", "", 0);
  expect(window.history.state).toMatchObject({ page: "community", tab: "community" });
});

// 切到「词库」是完整的社区词库列表；离开「社区」标签再回来，仍是上次选的分类。
test("switching to 词库 shows the community dictionary list and the choice is remembered", async () => {
  const resources = resourcesClient([
    dictionaryResource("10000000-0000-4000-8000-000000000301", "合成词库"),
  ]);
  renderPhone("community", { resources });
  const tabs = await screen.findByRole("tablist", { name: "社区分类" });

  fireEvent.click(within(tabs).getByRole("tab", { name: "词库" }));
  const panel = screen.getByRole("tabpanel", { name: "词库" });
  expect(screen.queryByRole("tabpanel", { name: "皮肤" })).toBeNull();
  await waitFor(() => expect(resources.list).toHaveBeenCalledWith("dictionary", "", "", 0));
  expect(await within(panel).findByRole("button", { name: "查看词库 合成词库" })).toBeTruthy();
  expect(within(panel).getByRole("search")).toBeTruthy();
  expect(panel.closest("form")).toBeNull();

  const bar = screen.getByRole("navigation", { name: "主要功能" });
  fireEvent.click(within(bar).getByRole("button", { name: "我的" }));
  expect(await screen.findByRole("heading", { level: 1, name: "我的" })).toBeTruthy();
  fireEvent.click(within(bar).getByRole("button", { name: "社区" }));
  expect(
    within(await screen.findByRole("tablist", { name: "社区分类" }))
      .getByRole("tab", { name: "词库" })
      .getAttribute("aria-selected"),
  ).toBe("true");
});

// 鸿蒙手机词库页「发现词库」末尾的「浏览更多词库」同样切到「社区」标签的「词库」分类。
test("the HarmonyOS phone's 浏览更多词库 switches to the 社区 tab's 词库 category", async () => {
  const resources = resourcesClient([
    dictionaryResource("10000000-0000-4000-8000-000000000302", "发现的词库"),
  ]);
  render(
    <SettingsPage
      initialPage="dictionary"
      client={{
        load: vi.fn().mockResolvedValue(initial),
        save: vi.fn().mockResolvedValue(initial),
        host: testHost({ platform: "harmony" }),
        home: {},
        account: account(),
        communitySkins: skinsClient(),
        communityResources: resources,
        dictionaryCollections: {
          load: vi.fn().mockResolvedValue({ collections: [], formats: [] }),
          flush: vi.fn().mockResolvedValue({ collections: [], formats: [] }),
        } as never,
      }}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "浏览更多词库" }));
  expect(await screen.findByRole("heading", { level: 1, name: "社区" })).toBeTruthy();
  expect(
    within(screen.getByRole("tablist", { name: "社区分类" }))
      .getByRole("tab", { name: "词库" })
      .getAttribute("aria-selected"),
  ).toBe("true");
  expect(screen.queryByRole("dialog", { name: "社区词库" })).toBeNull();
});

test("离开回复模板区后旧添加完成不弹出迟到提示", async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const resources = resourcesClient(replies);
  resources.save = vi.fn().mockReturnValue(pending);
  renderPhone("ai", { resources });
  const group = await screen.findByRole("region", { name: "回复模板" });
  fireEvent.click(await within(group).findByRole("button", { name: "添加回复模板 回复模板1" }));
  fireEvent.click(
    within(screen.getByRole("navigation", { name: "主要功能" })).getByRole("button", {
      name: "我的",
    }),
  );
  expect(screen.queryByRole("region", { name: "回复模板" })).toBeNull();
  await act(async () => {
    finish();
  });
  expect(resources.storeReply).toHaveBeenCalledWith(replies[0]);
  expect(screen.queryByText("已添加「回复模板1」到高情商回复键盘")).toBeNull();
});

test("同一批事件不能重复添加回复模板", async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const resources = resourcesClient(replies);
  resources.save = vi.fn().mockReturnValue(pending);
  renderPhone("ai", { resources });
  const group = await screen.findByRole("region", { name: "回复模板" });
  const add = await within(group).findByRole("button", { name: "添加回复模板 回复模板1" });
  act(() => {
    fireEvent.click(add);
    fireEvent.click(add);
  });
  await act(async () => {
    finish();
  });
  expect(resources.save).toHaveBeenCalledExactlyOnceWith(replies[0].id, true);
});
