import { expect, test } from "vitest";
import { mobilePageTitle } from "../../../../packages/ui/src/settings/mobile-tab-helpers";
import { settingsPageProjections } from "../../../../packages/ui/src/settings/settings-page-projections";

const project = (overrides: Partial<Parameters<typeof settingsPageProjections>[0]> = {}) =>
  settingsPageProjections({
    mobilePlatform: false,
    hasHomePage: true,
    hasTypingStatistics: true,
    hasVocabularyReview: true,
    hasAccount: true,
    hasChat: true,
    hasCommunity: true,
    showFloatingToolbar: true,
    showDeveloperPage: true,
    hasPlugins: true,
    hasHandwriting: true,
    hasMcp: true,
    mobileHiddenPageIds: ["floating-toolbar", "plugins"],
    mobilePageTitle,
    ...overrides,
  });

test("desktop hosts with a community client keep the 社区 page", () => {
  expect(project().availablePages.map((page) => page.id)).toContain("community");
});

test("filters pages by host capabilities while preserving registry order", () => {
  const desktop = project({
    hasHomePage: false,
    hasChat: false,
    hasCommunity: false,
    showFloatingToolbar: false,
  });

  expect(desktop.availablePages.map((page) => page.id)).not.toContain("home");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("chat");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("community");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("floating-toolbar");
  expect(desktop.availablePages.map((page) => page.id)).toEqual(
    expect.arrayContaining(["appearance", "input"]),
  );
});

const groupIds = (groups: readonly { title?: string; pages: readonly { id: string }[] }[]) =>
  groups.map((group) => ({ title: group.title, ids: group.pages.map((page) => page.id) }));

test("projects the desktop sidebar into the titled navigation groups", () => {
  expect(groupIds(project().sidebarGroups)).toEqual([
    { title: undefined, ids: ["home"] },
    { title: "打字", ids: ["input", "dictionary", "shortcuts", "expression"] },
    { title: "输入方式", ids: ["voice", "screen-keyboard", "handwriting"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "工具", ids: ["tools", "ai", "plugins"] },
    { title: "数据", ids: ["typing-statistics", "account"] },
    // 桌面上「帮助与反馈」并进「关于」，侧栏不再列出它；它仍是可路由的页面。
    { title: "支持", ids: ["developer", "about"] },
  ]);
  // 桌面标题直接说明功能，不限制字数；版本裁剪后不产生空入口。
  const desktop = project({
    hasHomePage: false,
    handwritingOnScreenKeyboardPage: true,
  });
  expect(desktop.sidebarGroups.flatMap((group) => group.pages.map((page) => page.title))).toEqual([
    "输入",
    "词库",
    "快捷键",
    "标点与翻译",
    "语音输入",
    "键盘与手写",
    "主题",
    "候选窗口",
    "悬浮工具栏",
    "剪贴板",
    "AI",
    "插件",
    "打字统计",
    "账号与同步",
    "维护与诊断",
    "关于",
  ]);
  expect(project().availablePages.map((page) => page.id)).toContain("feedback");
  // 宿主不提供的页从所在组里消失；整组为空时整组不出现。
  expect(
    groupIds(
      project({
        hasHomePage: false,
        hasAccount: false,
        hasCommunity: false,
        hasPlugins: false,
        hasTypingStatistics: false,
      }).sidebarGroups,
    ),
  ).toEqual([
    { title: "打字", ids: ["input", "dictionary", "shortcuts", "expression"] },
    { title: "输入方式", ids: ["voice", "screen-keyboard", "handwriting"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "工具", ids: ["tools", "ai"] },
    { title: "支持", ids: ["developer", "about"] },
  ]);
});

test("桌面 MCP 并入 AI 导航但保留能力条件路由，触屏仍独立", () => {
  const desktop = project();
  expect(desktop.availablePages.map((page) => page.id)).toContain("mcp");
  expect(
    desktop.sidebarGroups.flatMap((group) => group.pages.map((page) => page.id)),
  ).not.toContain("mcp");
  expect(desktop.availablePages.find((page) => page.id === "mcp")?.title).toBe("AI 助手");
  expect(project({ hasMcp: false }).availablePages.map((page) => page.id)).not.toContain("mcp");
});

test("桌面账号页覆盖账号与同步，触屏仍叫我的", () => {
  expect(project().availablePages.find((page) => page.id === "account")?.title).toBe("账号与同步");
  expect(
    project({ mobilePlatform: true }).availablePages.find((page) => page.id === "account")?.title,
  ).toBe("我的");
});

test("no longer offers 其他平台下载 as a page of its own", () => {
  expect(project().availablePages.map((page) => page.id)).not.toContain("download");
  expect(project({ mobilePlatform: true }).availablePages.map((page) => page.id)).not.toContain(
    "download",
  );
});

test("projects mobile pages into tabs, grouped settings, and sidebar sections", () => {
  const mobile = project({
    mobilePlatform: true,
    mobileHiddenPageIds: ["shortcuts", "floating-toolbar"],
  });

  expect(mobile.availablePages.find((page) => page.id === "appearance")?.title).toBe("候选栏");
  // 底部是 首页 / 社区 / 统计 / 我的（首页的根页以应用名作标题）：第二格是社区页，「全部设置」已去掉。
  expect(mobile.mobilePrimaryPages.map((page) => [page.id, page.title])).toEqual([
    ["home", "水杉输入法"],
    ["community", "社区"],
    ["typing-statistics", "统计"],
    ["account", "我的"],
  ]);
  expect(mobile.availablePages.map((page) => page.id)).toContain("community");
  const listed = mobile.mobileSecondaryGroups.flatMap((group) =>
    group.pages.map((page) => page.id),
  );
  // 社区有自己的标签页，「首页」列表不重复列它；皮肤页回到「首页」列表的「外观」组。
  for (const id of ["home", "chat", "community", "typing-statistics", "account", "shortcuts"]) {
    expect(listed).not.toContain(id);
  }
  expect(listed).not.toContain("floating-toolbar");
  // 有账号页的宿主在「我的」里放了关于和帮助与反馈，「首页」列表不再重复列出。
  expect(listed).not.toContain("about");
  expect(listed).not.toContain("feedback");
  expect(groupIds(mobile.mobileSecondaryGroups)).toEqual([
    { title: "打字", ids: ["input", "dictionary", "expression"] },
    { title: "键盘、语音与手写", ids: ["voice", "screen-keyboard", "handwriting"] },
    { title: "外观", ids: ["skin", "appearance"] },
    { title: "工具", ids: ["tools", "ai", "mcp", "plugins"] },
    { title: "支持", ids: ["developer"] },
  ]);
  expect(mobile.sidebarGroups[0].pages.map((page) => page.id)).toEqual(["home"]);
  expect(mobile.sidebarGroups.map((group) => group.title)).toContain("键盘、语音与手写");
  expect(mobile.sidebarGroups.map((group) => group.title)).not.toContain("输入方式");
  expect(mobile.sidebarGroups.flatMap((group) => group.pages.map((page) => page.id))).toContain(
    "skin",
  );
  expect(mobile.availablePages.map((page) => page.id)).not.toContain("more");
});

// 桌面侧栏照旧在「外观」组里列出皮肤（主题）页，触屏的标签页调整不影响它。
test("the desktop sidebar keeps the skin entry", () => {
  const desktop = project();
  expect(groupIds(desktop.sidebarGroups)).toContainEqual({
    title: "外观",
    ids: ["skin", "appearance", "floating-toolbar"],
  });
  expect(desktop.availablePages.find((page) => page.id === "skin")?.title).toBe("主题");
});

// AI 对话是「AI 辅助」的二级页，不占标签：没有对话客户端的触屏宿主底部仍是同样的四格。
test("a touch host without a chat client keeps the same four tabs", () => {
  const mobile = project({ mobilePlatform: true, hasChat: false });
  expect(mobile.mobilePrimaryPages.map((page) => page.id)).toEqual([
    "home",
    "community",
    "typing-statistics",
    "account",
  ]);
});

// 没有社区服务（既没有皮肤图库也没有词库图库）的触屏宿主没有「社区」这一格，而不是留一个打不开的标签页。
test("a touch host without a community client has no 社区 tab", () => {
  const mobile = project({ mobilePlatform: true, hasCommunity: false });
  expect(mobile.mobilePrimaryPages.map((page) => page.id)).toEqual([
    "home",
    "typing-statistics",
    "account",
  ]);
  expect(mobile.availablePages.map((page) => page.id)).not.toContain("community");
});

// iPad 与手机同样 mobilePlatform 为真：标签栏上的社区、统计、我的不在侧栏里重复出现，侧栏只在首页之后列出分组；皮肤页照常在「外观」组里。
test("keeps the tab pages out of the iPad sidebar", () => {
  const ipad = project({ mobilePlatform: true });
  const sidebar = ipad.sidebarGroups.flatMap((group) => group.pages.map((page) => page.id));
  expect(sidebar[0]).toBe("home");
  for (const id of ["chat", "community", "typing-statistics", "account"]) {
    expect(sidebar).not.toContain(id);
  }
  expect(sidebar).toContain("skin");
});

test("places the 插件 page in 工具 and drops it where the host does not back it", () => {
  const desktop = project();
  const group = desktop.sidebarGroups.find((item) =>
    item.pages.some((page) => page.id === "tools"),
  );
  expect(group?.title).toBe("工具");
  expect(group?.pages.map((page) => page.id)).toEqual(["tools", "ai", "plugins"]);
  expect(desktop.availablePages.find((page) => page.id === "plugins")?.title).toBe("插件");

  expect(project({ hasPlugins: false }).availablePages.map((page) => page.id)).not.toContain(
    "plugins",
  );
  const phone = project({ mobilePlatform: true });
  expect(phone.sidebarGroups.flatMap((item) => item.pages.map((page) => page.id))).not.toContain(
    "plugins",
  );
  expect(
    phone.mobileSecondaryGroups.flatMap((item) => item.pages.map((page) => page.id)),
  ).not.toContain("plugins");
});

// 日文、越南文和藏文版没有手写（版本表 features.handwriting 为 false）：桌面侧栏和手机「首页」列表都不列出手写页，同组的其他页照旧。
test("drops the 手写输入 page from an edition without handwriting", () => {
  const desktop = project({ hasHandwriting: false });
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("handwriting");
  expect(groupIds(desktop.sidebarGroups)).toContainEqual({
    title: "输入方式",
    ids: ["voice", "screen-keyboard"],
  });
  const phone = project({ mobilePlatform: true, hasHandwriting: false });
  expect(groupIds(phone.mobileSecondaryGroups)).toContainEqual({
    title: "键盘、语音与手写",
    ids: ["voice", "screen-keyboard"],
  });
});

// 触屏宿主按设计的叫法命名页面；桌面侧栏保留注册表里的标题。
test("renames pages to the design's touch titles on mobile only", () => {
  const title = (projection: ReturnType<typeof project>, id: string) =>
    projection.availablePages.find((page) => page.id === id)?.title;
  const mobile = project({ mobilePlatform: true });
  const desktop = project();
  for (const [id, touch, registry] of [
    ["home", "水杉输入法", "首页"],
    ["skin", "皮肤", "主题"],
    ["expression", "表达", "标点与翻译"],
    ["developer", "开发者选项", "维护与诊断"],
    ["typing-statistics", "统计", "打字统计"],
    ["feedback", "反馈", "帮助与反馈"],
  ] as const) {
    expect(title(mobile, id)).toBe(touch);
    expect(title(desktop, id)).toBe(registry);
  }
  // 「首页」根页的行也用同样的名字。
  const rows = mobile.mobileSecondaryGroups.flatMap((group) => group.pages);
  expect(rows.find((page) => page.id === "expression")?.title).toBe("表达");
  expect(rows.find((page) => page.id === "developer")?.title).toBe("开发者选项");
});

// 鸿蒙 2-in-1 外壳投影时不含首页，所以它的侧栏从「输入」开始。
test("a host without the home page starts its sidebar at 输入", () => {
  const sidebar = project({ hasHomePage: false }).sidebarGroups;
  expect(sidebar[0].title).toBe("打字");
  expect(sidebar[0].pages[0].id).toBe("input");
  expect(sidebar.flatMap((group) => group.pages.map((page) => page.id))).not.toContain("home");
});

// 有对话的触屏宿主在「对话」标签里试键盘，独立的试用页不再注册，历史记录里留下的这一页也会落回首页。
test("a touch host offers the 试用键盘 sub-page only when it has no chat", () => {
  const ids = (hasChat: boolean) =>
    project({ mobilePlatform: true, hasChat }).availablePages.map((page) => page.id);
  expect(ids(true)).not.toContain("try-keyboard");
  expect(ids(false)).toContain("try-keyboard");
  expect(project().availablePages.map((page) => page.id)).not.toContain("try-keyboard");
});

// 桌面宿主上手写页只有一个打开按钮或一段说明，并进屏幕键盘页：侧栏少一项，屏幕键盘页改叫「屏幕键盘与手写」，旧的 handwriting id 指向它。
test("folds 手写输入 into the screen keyboard page where the host asks for it", () => {
  const merged = project({ handwritingOnScreenKeyboardPage: true });
  expect(merged.handwritingOnScreenKeyboardPage).toBe(true);
  expect(merged.availablePages.map((page) => page.id)).not.toContain("handwriting");
  expect(merged.availablePages.find((page) => page.id === "screen-keyboard")?.title).toBe(
    "键盘与手写",
  );
  expect(groupIds(merged.sidebarGroups)).toContainEqual({
    title: "输入方式",
    ids: ["voice", "screen-keyboard"],
  });
  expect(merged.pageAliases).toEqual({ handwriting: "screen-keyboard" });
  // 同一个宿主两次投影拿到同一个别名对象，依赖它的 effect 不会每次渲染都重跑。
  expect(project({ handwritingOnScreenKeyboardPage: true }).pageAliases).toBe(merged.pageAliases);

  // Windows（与原生设置窗口的 SettingsNavigation.h 一致）和默认情况照旧分两页。
  const separate = project();
  expect(separate.handwritingOnScreenKeyboardPage).toBe(false);
  expect(separate.availablePages.find((page) => page.id === "screen-keyboard")?.title).toBe(
    "屏幕键盘",
  );
  expect(separate.pageAliases).toEqual({});

  // 触屏宿主的手写页另有内容，不并。
  const phone = project({ mobilePlatform: true, handwritingOnScreenKeyboardPage: true });
  expect(phone.handwritingOnScreenKeyboardPage).toBe(false);
  expect(phone.availablePages.map((page) => page.id)).toContain("handwriting");

  // 本版本没有手写时没有可并的页，屏幕键盘页保留原名。
  const noHandwriting = project({ hasHandwriting: false, handwritingOnScreenKeyboardPage: true });
  expect(noHandwriting.handwritingOnScreenKeyboardPage).toBe(false);
  expect(noHandwriting.availablePages.find((page) => page.id === "screen-keyboard")?.title).toBe(
    "屏幕键盘",
  );
  expect(noHandwriting.pageAliases).toEqual({});
});
