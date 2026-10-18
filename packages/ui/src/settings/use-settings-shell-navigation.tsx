import { useState, type MutableRefObject } from "react";
import type { SettingsPageGroup, SettingsPageItem } from "./settings-page-projections";
import { mobileTabForPage } from "./settings-navigation-helpers";
import { pages, type SettingsPageId } from "./settings-page-registry";
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

const untitledSidebarGroupKey = "__untitled__";

function sidebarGroupKey(group: SettingsPageGroup): string {
  return group.title ?? untitledSidebarGroupKey;
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
          // 菜单缩短名称后，仍能用「AI」「快捷键」「工具栏」等原名找到页面。
          pages: group.pages.filter((item) =>
            [item.title, pages.find((page) => page.id === item.id)?.title ?? ""].some((title) =>
              title.toLocaleLowerCase().includes(navNeedle),
            ),
          ),
        }))
        .filter((group) => group.pages.length > 0)
    : sidebarGroups;
  const [collapsedSidebarGroups, setCollapsedSidebarGroups] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const isSidebarGroupExpanded = (group: SettingsPageGroup): boolean => {
    if (!group.title) return true;
    // 搜索结果和当前页所在组始终可见，避免折叠状态把导航目标藏起来。
    if (navNeedle || group.pages.some((item) => item.id === navigationPage)) return true;
    return !collapsedSidebarGroups.has(sidebarGroupKey(group));
  };
  const toggleSidebarGroup = (group: SettingsPageGroup) => {
    if (!group.title) return;
    const key = sidebarGroupKey(group);
    setCollapsedSidebarGroups((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };
  const navSearchField = (
    <SettingsNavigationSearch
      groups={shownSidebarGroups}
      value={navQuery}
      onChange={setNavQuery}
      onSelectPage={selectPage}
    />
  );

  return {
    parentPage,
    goBack,
    shownSidebarGroups,
    navSearchField,
    isSidebarGroupExpanded,
    toggleSidebarGroup,
  } as const;
}
