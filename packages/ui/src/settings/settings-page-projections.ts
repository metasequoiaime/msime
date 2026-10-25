import { mobilePrimaryPageIds, type MobilePrimaryPageId } from "./settings-navigation-helpers";
import {
  desktopMergedPages,
  pages,
  settingsNavGroups,
  type SettingsPageId,
} from "./settings-page-registry";
import { mobileGroupTitle } from "./mobile-tab-helpers";

export interface SettingsPageProjectionOptions {
  mobilePlatform: boolean;
  hasHomePage: boolean;
  hasTypingStatistics: boolean;
  hasVocabularyReview: boolean;
  hasAccount: boolean;
  hasChat: boolean;
  hasCommunity: boolean;
  showFloatingToolbar: boolean;
  showDeveloperPage: boolean;
  /** The 插件 page: a host with a pack store, or one that plays or routes something it switches. */
  hasPlugins: boolean;
  /** 「手写输入」页：手写识别器只认汉字，不提供手写的版本（`EditionInfo.handwriting` 为 false）没有这一页。 */
  hasHandwriting: boolean;
  /** 「手写输入」并进「屏幕键盘」页，那一页改叫「键盘与手写」。macOS、Linux、Windows 和 HarmonyOS 2-in-1 上手写页只有一个打开按钮或一段说明，单占一个侧栏入口不值得，Windows 原生设置窗口的 `SettingsNavigation.h` 也同样并成一页；触屏宿主的手写页另有内容，不并。本版本不提供手写时没有可并的页。 */
  handwritingOnScreenKeyboardPage?: boolean;
  /** 宿主能报告本机 MCP 服务状态时，提供 MCP 连接内容；桌面通过「AI」顶部标签进入。 */
  hasMcp: boolean;
  mobileHiddenPageIds: readonly SettingsPageId[];
  mobilePageTitle: (id: SettingsPageId, title: string) => string;
}

export interface SettingsPageItem {
  readonly id: SettingsPageId;
  readonly title: string;
  readonly icon: string;
}

/** 导航里的一组页面。`title` 为空的组不显示组名，例如侧栏最前面单独的「首页」。 */
export interface SettingsPageGroup {
  readonly title?: string;
  readonly pages: SettingsPageItem[];
}

export interface SettingsPageProjections {
  availablePages: SettingsPageItem[];
  sidebarGroups: SettingsPageGroup[];
  mobilePrimaryPages: SettingsPageItem[];
  mobileSecondaryGroups: SettingsPageGroup[];
  /** 已经并进别的页面的页 id 指向现在承载它的页。旧路由（例如 `settings:handwriting`）和记住的页面经它打开合并后的页，而不是落到默认页。 */
  pageAliases: Partial<Record<SettingsPageId, SettingsPageId>>;
  /** 「手写输入」这次确实并进了「屏幕键盘」页：页面据此在屏幕键盘页里画手写的分组。 */
  handwritingOnScreenKeyboardPage: boolean;
}

/** 合并后「屏幕键盘」页的标题。 */
export const screenKeyboardAndHandwritingTitle = "键盘与手写";

// 模块级常量，每次渲染拿到同一个对象，依赖它的 effect 不会白白重跑。
const handwritingAliases: Partial<Record<SettingsPageId, SettingsPageId>> = {
  handwriting: "screen-keyboard",
};
const noPageAliases: Partial<Record<SettingsPageId, SettingsPageId>> = {};

/** Projects the current registry into the host-aware page lists used by the settings shell. */
export function settingsPageProjections({
  mobilePlatform,
  hasHomePage,
  hasTypingStatistics,
  hasVocabularyReview,
  hasAccount,
  hasChat,
  hasCommunity,
  showFloatingToolbar,
  showDeveloperPage,
  hasPlugins,
  hasHandwriting,
  handwritingOnScreenKeyboardPage: combineHandwriting = false,
  hasMcp,
  mobileHiddenPageIds,
  mobilePageTitle,
}: SettingsPageProjectionOptions): SettingsPageProjections {
  const handwritingOnScreenKeyboardPage = combineHandwriting && hasHandwriting && !mobilePlatform;
  const availablePages = pages
    .filter(
      (item) =>
        (item.id !== "home" || hasHomePage) &&
        (item.id !== "typing-statistics" || hasTypingStatistics) &&
        (item.id !== "vocabulary" || hasVocabularyReview) &&
        (item.id !== "account" || hasAccount) &&
        (item.id !== "chat" || hasChat) &&
        // 有社区服务的宿主才有「社区」页。触屏宿主上它是底部「社区」标签的根页（皮肤与词库两类图库）；桌面上它不在侧栏里，只从「我的内容」各行进入。
        (item.id !== "community" || hasCommunity) &&
        (item.id !== "floating-toolbar" || showFloatingToolbar) &&
        (item.id !== "developer" || showDeveloperPage) &&
        (item.id !== "plugins" || hasPlugins) &&
        (item.id !== "handwriting" || (hasHandwriting && !handwritingOnScreenKeyboardPage)) &&
        (item.id !== "mcp" || hasMcp) &&
        // 「隐私」页是触屏「我的」里那一行打开的；桌面的隐私设置在「关于」里。
        (item.id !== "privacy" || mobilePlatform) &&
        // 触屏宿主从「AI 辅助」进入二级对话，独立试用页只留给没有对话客户端的宿主。
        (item.id !== "try-keyboard" || (mobilePlatform && hasHomePage && !hasChat)),
    )
    .map((item) => ({
      ...item,
      title: mobilePlatform
        ? mobilePageTitle(item.id, item.title)
        : item.id === "ai"
          ? "AI"
          : handwritingOnScreenKeyboardPage && item.id === "screen-keyboard"
            ? screenKeyboardAndHandwritingTitle
            : item.title,
    }));

  // 触屏的「首页」列表（和 iPad 侧栏）不列底部标签页自己的根页：社区、统计、我的已经各占一格。桌面和 2in1 的侧栏不经过这条过滤。
  const mobileListedPage = (id: SettingsPageId): boolean =>
    ((id !== "about" && id !== "feedback") || !hasAccount) &&
    !mobilePrimaryPageIds.includes(id as MobilePrimaryPageId) &&
    !mobileHiddenPageIds.includes(id);

  // 桌面侧栏不列出并进别的页的页面（`desktopMergedPages`）；它们仍在 `availablePages` 里，路由打开时显示承载页。
  const sidebarPages = availablePages.filter(
    (item) =>
      !(mobilePlatform && mobileHiddenPageIds.includes(item.id)) &&
      (mobilePlatform || desktopMergedPages[item.id] === undefined),
  );
  const byId = new Map(sidebarPages.map((item) => [item.id, item]));
  const groups = settingsNavGroups
    .map(({ title, ids }) => ({
      title: mobilePlatform ? mobileGroupTitle(title) : title,
      pages: ids.flatMap((id) => {
        if (mobilePlatform && !mobileListedPage(id)) return [];
        const item = byId.get(id);
        return item ? [item] : [];
      }),
    }))
    .filter((group) => group.pages.length > 0);
  const home = byId.get("home");
  const sidebarGroups = home ? [{ pages: [home] }, ...groups] : groups;

  const mobilePrimaryPages = mobilePrimaryPageIds.flatMap((id) => {
    const item = availablePages.find((page) => page.id === id);
    return item ? [item] : [];
  });
  const mobileSecondaryGroups = settingsNavGroups
    .map(({ title, ids }) => ({
      title: mobileGroupTitle(title),
      pages: ids.flatMap((id) => {
        if (!mobileListedPage(id)) return [];
        const item = availablePages.find((page) => page.id === id);
        return item ? [item] : [];
      }),
    }))
    .filter((group) => group.pages.length > 0);

  const pageAliases = handwritingOnScreenKeyboardPage ? handwritingAliases : noPageAliases;

  return {
    availablePages,
    sidebarGroups,
    mobilePrimaryPages,
    mobileSecondaryGroups,
    pageAliases,
    handwritingOnScreenKeyboardPage,
  };
}
