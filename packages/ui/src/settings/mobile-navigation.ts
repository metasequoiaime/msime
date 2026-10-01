import {
  mobilePrimaryPageIds,
  requestedPage as resolveRequestedPage,
  type MobilePrimaryPageId,
} from "./settings-navigation-helpers";
import { pages, settingsPageAliases, type SettingsPageId } from "./settings-page-registry";

export {
  mobilePrimaryPageIds,
  mobileTabForPage,
  type MobilePrimaryPageId,
} from "./settings-navigation-helpers";

/**
 * 旧的页面列表导出。页面、顺序、标题和图标都直接取自 `settings-page-registry`，不再单独维护第二份注册表；手机上的页名由 `mobilePageTitle` 在投影时改写。
 */
export { pages, type SettingsPageId } from "./settings-page-registry";

/** Primary mobile pages whose content already supplies its own heading. */
export const mobileHeaderlessPageIds: readonly SettingsPageId[] = [
  "home",
  "typing-statistics",
  "account",
];

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

/** 宿主请求的页面：旧 id 先经 `settingsPageAliases` 改道，未知 id 落到导航第一页「输入」。 */
export function requestedPage(value: string | undefined): SettingsPageId {
  return resolveRequestedPage(value, pages, settingsPageAliases, "input");
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
