import type { MutableRefObject } from "react";
import type { SettingsPageGroup, SettingsPageItem } from "./settings-page-projections";
import { mobileTabForPage } from "./settings-navigation-helpers";
import type { SettingsPageId } from "./settings-page-registry";
import { SettingsNavigationSearch } from "./settings-navigation-search";

export interface UseSettingsShellNavigationOptions {
  page: SettingsPageId;
  navigationPage: SettingsPageId;
  launchPage: MutableRefObject<SettingsPageId>;
  availablePages: readonly SettingsPageItem[];
  sidebarGroups: readonly SettingsPageGroup[];
  navQuery: string;
  setNavQuery: (value: string) => void;
  selectPage: (page: SettingsPageId, replace?: boolean) => void;
}

/** 管理设置外壳的返回出口、父页面和侧栏搜索投影。 */
export function useSettingsShellNavigation({
  page,
  navigationPage,
  launchPage,
  availablePages,
  sidebarGroups,
  navQuery,
  setNavQuery,
  selectPage,
}: UseSettingsShellNavigationOptions) {
  const parentPage =
    navigationPage !== page ? availablePages.find((item) => item.id === navigationPage) : undefined;

  // 设置打开时所在的页面身后没有设置历史条目，启动页上的返回按钮要替换当前记录；推入页则沿历史返回。
  const goBack = () => {
    if (page !== launchPage.current) {
      window.history.back();
      return;
    }
    const up = parentPage?.id ?? mobileTabForPage(page);
    launchPage.current = up;
    selectPage(up, true);
  };

  const navNeedle = navQuery.trim().toLocaleLowerCase();
  const shownSidebarGroups = navNeedle
    ? sidebarGroups
        .map((group) => ({
          ...group,
          pages: group.pages.filter((item) => item.title.toLocaleLowerCase().includes(navNeedle)),
        }))
        .filter((group) => group.pages.length > 0)
    : sidebarGroups;
  const navSearchField = (
    <SettingsNavigationSearch
      groups={shownSidebarGroups}
      value={navQuery}
      onChange={setNavQuery}
      onSelectPage={selectPage}
    />
  );

  return { parentPage, goBack, shownSidebarGroups, navSearchField } as const;
}
