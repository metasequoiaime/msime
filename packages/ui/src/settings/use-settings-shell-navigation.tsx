import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MutableRefObject,
  type RefObject,
} from "react";
import {
  collectSettingsSearchTargets,
  revealSettingsSearchTarget,
  focusSettingsSearchTarget,
  type SettingsSearchTarget,
} from "./settings-search-index";
import type { SettingsPageGroup, SettingsPageItem } from "./settings-page-projections";
import { mobileTabForPage } from "./settings-navigation-helpers";
import { pages, type SettingsPageId } from "./settings-page-registry";
import { SettingsNavigationSearch } from "./settings-navigation-search";

export interface UseSettingsShellNavigationOptions {
  contentRef?: RefObject<HTMLElement | null>;
  onRevealPage?: (page: SettingsPageId) => void;
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
  contentRef,
  onRevealPage,
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
          // 显示名和注册名都参与搜索，合并后的「键盘与手写」仍能按「屏幕键盘」查找。
          pages: group.pages.filter((item) =>
            [
              item.title,
              ...pages
                .filter(
                  (page) =>
                    page.id === item.id ||
                    (page.id === "mcp" &&
                      item.id === "ai" &&
                      availablePages.some((available) => available.id === page.id)),
                )
                .map((page) => page.title),
            ].some((title) => title.toLocaleLowerCase().includes(navNeedle)),
          ),
        }))
        .filter((group) => group.pages.length > 0)
    : sidebarGroups;
  const [targets, setTargets] = useState<SettingsSearchTarget[]>([]);
  const [active, setActive] = useState(0);
  const pending = useRef<SettingsSearchTarget | null>(null);
  const listId = useId();
  const pageTitlesKey = JSON.stringify(availablePages.map((item) => [item.id, item.title]));
  const pageTitles = useMemo(
    () => new Map<SettingsPageId, string>(JSON.parse(pageTitlesKey)),
    [pageTitlesKey],
  );
  useLayoutEffect(() => {
    setActive(0);
    const root = contentRef?.current;
    if (!root || !navNeedle) {
      setTargets((previous) => (previous.length ? [] : previous));
      return;
    }
    const update = () => setTargets(collectSettingsSearchTargets(root, pageTitles));
    update();
    const observer = new MutationObserver(update);
    observer.observe(root, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["hidden", "data-settings-search-label"],
    });
    return () => observer.disconnect();
  }, [contentRef, navNeedle, pageTitles]);
  const matching = targets.filter((target) => target.title.toLocaleLowerCase().includes(navNeedle));
  const results = [
    ...shownSidebarGroups.flatMap((group) =>
      group.pages.map((item) => ({
        page: item.id,
        title: item.title,
        path: [] as string[],
        target: undefined as SettingsSearchTarget | undefined,
      })),
    ),
    ...matching.map((target) => ({ ...target, target })),
  ];
  const selected = Math.min(active, Math.max(0, results.length - 1));
  const openResult = (index: number) => {
    const result = results[index];
    if (!result) return;
    pending.current = result.target ?? null;
    selectPage(result.page);
    if (result.target) onRevealPage?.(result.page);
    setNavQuery("");
  };
  useLayoutEffect(() => {
    const target = pending.current;
    if (!target || page !== target.page) return;
    pending.current = null;
    revealSettingsSearchTarget(target);
    // 等标签与下钻状态提交后聚焦；两帧也让页面自己的滚动复位先完成。
    let focusFrame = 0;
    const frame = requestAnimationFrame(() => {
      focusFrame = requestAnimationFrame(() => focusSettingsSearchTarget(target));
    });
    return () => {
      cancelAnimationFrame(frame);
      cancelAnimationFrame(focusFrame);
    };
  }, [page, navQuery]);
  useEffect(() => {
    if (navNeedle)
      document.getElementById(`${listId}-${selected}`)?.scrollIntoView?.({ block: "nearest" });
  }, [listId, selected, navNeedle]);
  const navSearchResults = navNeedle ? (
    <div id={listId} role="listbox" aria-label="设置搜索结果">
      {results.map((result, index) => (
        <button
          type="button"
          role="option"
          tabIndex={-1}
          aria-selected={selected === index}
          id={`${listId}-${index}`}
          key={`${result.page}-${index}`}
          className="mb-1 flex w-full appearance-none flex-col gap-1 rounded-lg border-0 bg-transparent px-3 py-2 text-left text-[13px] text-[var(--p-text)] [font-family:inherit] aria-selected:bg-accent/15 hover:bg-accent/10 focus-visible:outline-2 focus-visible:outline-accent"
          onClick={() => openResult(index)}
        >
          <span>{result.title}</span>
          {result.path.length > 0 && (
            <span className="text-xs opacity-60">{result.path.join(" › ")}</span>
          )}
        </button>
      ))}
      {results.length === 0 && (
        <p role="status" className="px-3 py-2 text-sm opacity-60">
          没有匹配的设置
        </p>
      )}
    </div>
  ) : undefined;
  const navSearchField = (
    <SettingsNavigationSearch
      resultNavigation={
        contentRef && navNeedle
          ? { active: selected, count: results.length, listId, setActive, select: openResult }
          : undefined
      }
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
    navSearchResults,
  } as const;
}
