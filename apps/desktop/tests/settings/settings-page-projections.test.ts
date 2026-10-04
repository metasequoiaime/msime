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
    mobileHiddenPageIds: ["floating-toolbar", "plugins"],
    mobilePageTitle,
    ...overrides,
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
    { title: "打字", ids: ["input", "expression", "shortcuts", "dictionary"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "更多输入方式", ids: ["screen-keyboard", "voice", "handwriting"] },
    { title: "工具", ids: ["tools", "typing-statistics", "plugins", "ai"] },
    { title: "账号", ids: ["account"] },
    { title: "支持", ids: ["developer", "feedback", "about"] },
  ]);
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
    { title: "打字", ids: ["input", "expression", "shortcuts", "dictionary"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "更多输入方式", ids: ["screen-keyboard", "voice", "handwriting"] },
    { title: "工具", ids: ["tools", "ai"] },
    { title: "支持", ids: ["developer", "feedback", "about"] },
  ]);
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
  expect(mobile.mobilePrimaryPages.map((page) => page.id)).toEqual([
    "home",
    "community",
    "typing-statistics",
    "account",
  ]);
  const listed = mobile.mobileSecondaryGroups.flatMap((group) =>
    group.pages.map((page) => page.id),
  );
  for (const id of ["home", "community", "typing-statistics", "account", "shortcuts"]) {
    expect(listed).not.toContain(id);
  }
  expect(listed).not.toContain("floating-toolbar");
  // 有账号页的宿主在「我的」里放了关于和帮助与反馈，「全部设置」不再重复列出。
  expect(listed).not.toContain("about");
  expect(listed).not.toContain("feedback");
  expect(groupIds(mobile.mobileSecondaryGroups)).toEqual([
    { title: "打字", ids: ["input", "expression", "dictionary"] },
    { title: "外观", ids: ["skin", "appearance"] },
    { title: "键盘、语音与手写", ids: ["screen-keyboard", "voice", "handwriting"] },
    { title: "工具", ids: ["tools", "plugins", "ai"] },
    { title: "支持", ids: ["developer"] },
  ]);
  expect(mobile.sidebarGroups[0].pages.map((page) => page.id)).toEqual(["home"]);
  expect(mobile.sidebarGroups.map((group) => group.title)).toContain("键盘、语音与手写");
  expect(mobile.sidebarGroups.map((group) => group.title)).not.toContain("更多输入方式");
  expect(mobile.sidebarGroups.flatMap((group) => group.pages.map((page) => page.id))).not.toContain(
    "more",
  );
});

// iPad 与手机同样 mobilePlatform 为真：标签栏上的社区、统计、我的不在侧栏里重复出现，侧栏只在首页之后列出分组。
test("keeps the tab pages out of the iPad sidebar", () => {
  const ipad = project({ mobilePlatform: true });
  const sidebar = ipad.sidebarGroups.flatMap((group) => group.pages.map((page) => page.id));
  expect(sidebar[0]).toBe("home");
  for (const id of ["community", "typing-statistics", "account"]) {
    expect(sidebar).not.toContain(id);
  }
});

test("places the 插件 page in 工具 and drops it where the host does not back it", () => {
  const desktop = project();
  const group = desktop.sidebarGroups.find((item) =>
    item.pages.some((page) => page.id === "tools"),
  );
  expect(group?.title).toBe("工具");
  expect(group?.pages.map((page) => page.id)).toEqual([
    "tools",
    "typing-statistics",
    "plugins",
    "ai",
  ]);
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

// 日文、越南文和藏文版没有手写（版本表 features.handwriting 为 false）：桌面侧栏和手机「全部设置」都不列出手写页，同组的其他页照旧。
test("drops the 手写输入 page from an edition without handwriting", () => {
  const desktop = project({ hasHandwriting: false });
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("handwriting");
  expect(groupIds(desktop.sidebarGroups)).toContainEqual({
    title: "更多输入方式",
    ids: ["screen-keyboard", "voice"],
  });
  const phone = project({ mobilePlatform: true, hasHandwriting: false });
  expect(groupIds(phone.mobileSecondaryGroups)).toContainEqual({
    title: "键盘、语音与手写",
    ids: ["screen-keyboard", "voice"],
  });
});
