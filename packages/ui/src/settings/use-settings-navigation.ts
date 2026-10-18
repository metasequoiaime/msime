import { createMobileSettingsHistoryState } from "./mobile-history-state";
import {
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import type { AccountCommunityDestination } from "../account/account-model";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
  type MobilePrimaryPageId,
} from "./settings-navigation-helpers";
import { desktopPage, subPageParents, type SettingsPageId } from "./settings-page-registry";
import { useMobilePopState } from "./use-mobile-pop-state";

export interface SettingsNavigationOptions {
  mobilePlatform: boolean;
  mobileHiddenPageIds: readonly SettingsPageId[];
  availablePages: readonly { id: SettingsPageId }[];
  /** 合并到其他页的旧 id 映射到当前承载页。 */
  pageAliases?: Partial<Record<SettingsPageId, SettingsPageId>>;
  page: SettingsPageId;
  setPage: Dispatch<SetStateAction<SettingsPageId>>;
  mobileLastPageByTab: MutableRefObject<Record<MobilePrimaryPageId, SettingsPageId>>;
  route?: { page: string; nonce: number };
  hasHomePage: boolean;
  setCommunityDestination: Dispatch<SetStateAction<AccountCommunityDestination | "all">>;
  setAccountLoginReturnPage: Dispatch<SetStateAction<SettingsPageId | null>>;
  accountLoginReturnPage: SettingsPageId | null;
}

const noPageAliases: Partial<Record<SettingsPageId, SettingsPageId>> = {};

/** 管理移动设置页的历史记录、标签记忆与外部路由。 */
export function useSettingsNavigation({
  mobilePlatform,
  mobileHiddenPageIds,
  availablePages,
  pageAliases = noPageAliases,
  page,
  setPage,
  mobileLastPageByTab,
  route,
  hasHomePage,
  setCommunityDestination,
  setAccountLoginReturnPage,
  accountLoginReturnPage,
}: SettingsNavigationOptions) {
  const [mobileActiveTab, setMobileActiveTab] = useState<MobilePrimaryPageId>(() =>
    mobileTabForPage(page),
  );
  const loginReturnTab = useRef<MobilePrimaryPageId>(mobileActiveTab);
  const handledRoute = useRef(route?.nonce);

  /** 打开 `next`。在移动端它是一条新的 WebView 历史记录，因此系统返回手势可以返回；`replace` 则用它替换当前记录，用于从背后没有设置记录的页面向上返回。 */
  const selectPage = (
    requested: SettingsPageId,
    replace = false,
    sourceTab?: MobilePrimaryPageId,
  ) => {
    const next = pageAliases[requested] ?? requested;
    if (mobilePlatform && mobileHiddenPageIds.includes(next)) return;
    const owner =
      sourceTab ??
      (mobilePrimaryPageIds.includes(next as MobilePrimaryPageId)
        ? (next as MobilePrimaryPageId)
        : mobileActiveTab);
    if (next === page && owner === mobileActiveTab) {
      // 同页路由仍需恢复子标签，例如再次打开反馈入口时返回反馈标签。
      setPage(next);
      return;
    }
    if (page === "account" && next !== "account") setAccountLoginReturnPage(null);
    if (mobilePlatform) {
      mobileLastPageByTab.current[owner] = next;
      setMobileActiveTab(owner);
    }
    setPage(next);
    if (mobilePlatform && typeof window !== "undefined") {
      const current = window.history.state;
      const state = createMobileSettingsHistoryState(current, {
        page: next,
        tab: owner,
      });
      delete state.panel;
      if (replace) window.history.replaceState(state, "");
      else window.history.pushState(state, "");
    }
    if (next === "community") setCommunityDestination("all");
  };

  useEffect(() => {
    if (!route || route.nonce === handledRoute.current) return;
    handledRoute.current = route.nonce;
    // 宿主未提供的路由回到输入页，并恢复该页默认的标签归属。
    const next = requestedPage(
      pageAliases[route.page as SettingsPageId] ?? route.page,
      availablePages,
      "input",
    );
    selectPage(next, false, mobileTabForPage(next));
    // Route nonces intentionally provide the effect's identity; the page callback reads current state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [route?.nonce]);

  // Mobile hosts use the WebView history stack for the system back gesture. The native activity
  // can therefore dismiss a nested page without the shared UI knowing which platform API is in use.
  useEffect(() => {
    if (!mobilePlatform || typeof window === "undefined") return;
    const current = window.history.state;
    if (!current || current.msimeSettings !== true) {
      window.history.replaceState(
        createMobileSettingsHistoryState(current, {
          page,
          tab: mobileActiveTab,
        }),
        "",
      );
    }
  }, [mobilePlatform]);

  useMobilePopState(mobilePlatform, (event) => {
    const state = event.state;
    if (state?.msimeSettings !== true || typeof state.page !== "string") return;
    const restored = requestedPage(state.page, availablePages, "input");
    if (restored !== "account") setAccountLoginReturnPage(null);
    const owner = mobilePrimaryPageIds.includes(state.tab as MobilePrimaryPageId)
      ? (state.tab as MobilePrimaryPageId)
      : mobileTabForPage(restored);
    mobileLastPageByTab.current[owner] = restored;
    setMobileActiveTab(owner);
    setPage(restored);
  });

  const selectMobileTab = (tab: SettingsPageId) => {
    if (!mobilePrimaryPageIds.includes(tab as MobilePrimaryPageId)) return;
    const primary = tab as MobilePrimaryPageId;
    if (primary === "account") setAccountLoginReturnPage(null);
    // 已经在这个标签里时再点它，回到这个标签的首页，和常见的手机应用一样；从别的标签点过来才回到它记住的那一页。
    if (primary === mobileActiveTab) {
      selectPage(primary);
      return;
    }
    const remembered = mobileLastPageByTab.current[primary];
    const available = availablePages.some((item) => item.id === remembered);
    selectPage(
      available && !mobileHiddenPageIds.includes(remembered) ? remembered : primary,
      false,
      primary,
    );
  };

  const openAccountLogin = () => {
    if (mobilePlatform) {
      loginReturnTab.current = mobileActiveTab;
      setAccountLoginReturnPage(page);
    }
    selectPage("account");
  };

  const finishAccountLogin = () => {
    const previous = accountLoginReturnPage;
    setAccountLoginReturnPage(null);
    if (!previous) return;
    mobileLastPageByTab.current[loginReturnTab.current] = previous;
    setMobileActiveTab(loginReturnTab.current);
    setPage(previous);
    if (mobilePlatform && typeof window !== "undefined") window.history.back();
  };

  useEffect(() => {
    // 已合并的旧页面仍属于打开它的标签，不能在别名重定向时跳到其他标签。
    const alias = pageAliases[page];
    if (alias) {
      setPage(alias);
      return;
    }
    const pageAvailable =
      availablePages.some((item) => item.id === page) &&
      (!mobilePlatform || !mobileHiddenPageIds.includes(page));
    if (!pageAvailable) {
      const fallback = mobilePlatform && hasHomePage ? "home" : "input";
      setMobileActiveTab(mobileTabForPage(fallback));
      setPage(fallback);
    }
  }, [
    availablePages,
    hasHomePage,
    mobileHiddenPageIds,
    mobilePlatform,
    page,
    pageAliases,
    setPage,
  ]);

  // 根标签页在移动端保持自身；子页和桌面合并页都高亮承载它们的页面。
  const navigationPage: SettingsPageId =
    mobilePlatform && mobilePrimaryPageIds.includes(page as MobilePrimaryPageId)
      ? page
      : mobilePlatform
        ? (subPageParents[page] ?? page)
        : desktopPage(subPageParents[page] ?? page);

  return {
    mobileActiveTab,
    navigationPage,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
  };
}
