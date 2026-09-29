/**
 * The settings pages shared by the desktop sidebar and the mobile navigation.
 *
 * Mobile hosts expose four primary tabs; the remaining pages are reached from the keyboard tab's
 * settings list. Keeping the page registry and tab rules together prevents each host surface from
 * maintaining a slightly different route allowlist.
 */
export const pages = [
  { id: "home", title: "首页", icon: new URL("../assets/msime.svg", import.meta.url).href },
  { id: "account", title: "我的", icon: new URL("../assets/account.svg", import.meta.url).href },
  { id: "chat", title: "AI 对话", icon: new URL("../assets/help.svg", import.meta.url).href },
  {
    id: "community",
    title: "社区",
    icon: new URL("../assets/community.svg", import.meta.url).href,
  },
  {
    id: "typing-statistics",
    title: "打字统计",
    icon: new URL("../assets/statistics.svg", import.meta.url).href,
  },
  {
    id: "appearance",
    title: "外观",
    icon: new URL("../assets/appearance.svg", import.meta.url).href,
  },
  { id: "input", title: "输入", icon: new URL("../assets/input.svg", import.meta.url).href },
  {
    id: "helpcode",
    title: "辅助码",
    icon: new URL("../assets/helpcode.svg", import.meta.url).href,
  },
  {
    id: "shortcuts",
    title: "快捷键",
    icon: new URL("../assets/shortcut.svg", import.meta.url).href,
  },
  {
    id: "dictionary",
    title: "词库",
    icon: new URL("../assets/dictionary.svg", import.meta.url).href,
  },
  {
    id: "vocabulary",
    title: "背单词",
    icon: new URL("../assets/vocabulary.svg", import.meta.url).href,
  },
  { id: "skin", title: "皮肤", icon: new URL("../assets/skin.svg", import.meta.url).href },
  {
    id: "voice",
    title: "语音输入",
    icon: new URL("../assets/voice-input.svg", import.meta.url).href,
  },
  {
    id: "screen-keyboard",
    title: "屏幕键盘",
    icon: new URL("../assets/screen-keyboard.svg", import.meta.url).href,
  },
  {
    id: "handwriting",
    title: "手写识别板",
    icon: new URL("../assets/handwriting.svg", import.meta.url).href,
  },
  {
    id: "tools",
    title: "实用功能",
    icon: new URL("../assets/utilities.svg", import.meta.url).href,
  },
  { id: "ai", title: "AI 辅助", icon: new URL("../assets/ai.svg", import.meta.url).href },
  {
    id: "floating-toolbar",
    title: "悬浮工具栏",
    icon: new URL("../assets/floating-toolbar.svg", import.meta.url).href,
  },
  // Mobile only, and the one page that is a list of the other pages. The phone bar carries the
  // source's four tabs, so everything else is reached the way the source reaches it: through the
  // 键盘 tab, down one level, into a list.
  {
    id: "more",
    title: "全部设置",
    icon: new URL("../assets/utilities.svg", import.meta.url).href,
  },
  { id: "help", title: "帮助", icon: new URL("../assets/help.svg", import.meta.url).href },
  { id: "about", title: "关于", icon: new URL("../assets/about.svg", import.meta.url).href },
  {
    id: "feedback",
    title: "反馈",
    icon: new URL("../assets/feedback.svg", import.meta.url).href,
  },
] as const;

export type SettingsPageId = (typeof pages)[number]["id"];
export type MobilePrimaryPageId = Extract<
  SettingsPageId,
  "home" | "community" | "typing-statistics" | "account"
>;

export const mobilePrimaryPageIds: readonly MobilePrimaryPageId[] = [
  "home",
  "community",
  "typing-statistics",
  "account",
];

/** Primary mobile pages whose content already supplies its own heading. */
export const mobileHeaderlessPageIds: readonly SettingsPageId[] = [
  "home",
  "typing-statistics",
  "account",
];

export interface InitialSettingsPageOptions {
  initialPage?: string;
  mobilePlatform: boolean;
  mobileHistoryState?: unknown;
  hasHomePage: boolean;
}

/** Resolves the first settings page from an explicit route, mobile history, or host capabilities. */
export function initialSettingsPage({
  initialPage,
  mobilePlatform,
  mobileHistoryState,
  hasHomePage,
}: InitialSettingsPageOptions): SettingsPageId {
  const historyState =
    mobileHistoryState && typeof mobileHistoryState === "object"
      ? (mobileHistoryState as Record<string, unknown>)
      : undefined;
  const restoredMobilePage =
    mobilePlatform && historyState?.msimeSettings === true && typeof historyState.page === "string"
      ? historyState.page
      : undefined;
  return requestedPage(initialPage ?? restoredMobilePage ?? (hasHomePage ? "home" : undefined));
}

/** Seeds each mobile tab's remembered leaf with the current page in its owning tab. */
export function initialMobileTabPages(
  page: SettingsPageId,
): Record<MobilePrimaryPageId, SettingsPageId> {
  const initialTab = mobileTabForPage(page);
  return {
    home: initialTab === "home" ? page : "home",
    community: initialTab === "community" ? page : "community",
    "typing-statistics": initialTab === "typing-statistics" ? page : "typing-statistics",
    account: initialTab === "account" ? page : "account",
  };
}

export function splitMobilePages<T extends { id: string }>(
  availablePages: readonly T[],
  hiddenPageIds: readonly string[],
): { primary: T[]; secondary: T[] } {
  const primary = mobilePrimaryPageIds.flatMap((id) => {
    const item = availablePages.find((page) => page.id === id);
    return item ? [item] : [];
  });
  const secondary = availablePages.filter(
    (item) =>
      item.id !== "more" &&
      !mobilePrimaryPageIds.includes(item.id as MobilePrimaryPageId) &&
      !hiddenPageIds.includes(item.id),
  );
  return { primary, secondary };
}

export function mobileTabForPage(page: SettingsPageId): MobilePrimaryPageId {
  return mobilePrimaryPageIds.includes(page as MobilePrimaryPageId)
    ? (page as MobilePrimaryPageId)
    : "home";
}

/** A host-provided route opens a known section, while an unknown id falls back to appearance. */
export function requestedPage(value: string | undefined): SettingsPageId {
  return pages.some((page) => page.id === value) ? (value as SettingsPageId) : "appearance";
}

/** Push a nested mobile settings route while preserving the host's existing history state. */
export function pushMobileSettingsState(state: Record<string, unknown>): void {
  const current = window.history.state;
  window.history.pushState(
    {
      ...(current && typeof current === "object" ? current : {}),
      msimeSettings: true,
      ...state,
    },
    "",
  );
}
