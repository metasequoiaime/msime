import {
  useEffect,
  useRef,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import type { AccountCommunityDestination } from "../account/account-page";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
  type MobilePrimaryPageId,
} from "./settings-navigation-helpers";
import type { SettingsPageId } from "./settings-page-registry";

export interface SettingsNavigationOptions {
  mobilePlatform: boolean;
  mobileHiddenPageIds: readonly SettingsPageId[];
  availablePages: readonly { id: SettingsPageId }[];
  page: SettingsPageId;
  setPage: Dispatch<SetStateAction<SettingsPageId>>;
  mobileLastPageByTab: MutableRefObject<Record<MobilePrimaryPageId, SettingsPageId>>;
  route?: { page: string; nonce: number };
  hasHomePage: boolean;
  setCommunityDestination: Dispatch<SetStateAction<AccountCommunityDestination | "all">>;
  setAccountLoginReturnPage: Dispatch<SetStateAction<SettingsPageId | null>>;
  accountLoginReturnPage: SettingsPageId | null;
}

/** Owns mobile settings history, tab memory, and route-driven page selection. */
export function useSettingsNavigation({
  mobilePlatform,
  mobileHiddenPageIds,
  availablePages,
  page,
  setPage,
  mobileLastPageByTab,
  route,
  hasHomePage,
  setCommunityDestination,
  setAccountLoginReturnPage,
  accountLoginReturnPage,
}: SettingsNavigationOptions) {
  const mobileActiveTab = mobileTabForPage(page);
  const handledRoute = useRef(route?.nonce);
  // The history listener outlives a render, so it reads the pages through a ref rather than the list it was created with.
  const availablePagesRef = useRef(availablePages);
  availablePagesRef.current = availablePages;

  /** 打开 `next`。在移动端它是一条新的 WebView 历史记录，因此系统返回手势可以返回；`replace` 则用它替换当前记录，用于从背后没有设置记录的页面向上返回。 */
  const selectPage = (next: SettingsPageId, replace = false) => {
    if (mobilePlatform && mobileHiddenPageIds.includes(next)) return;
    if (next === page) return;
    if (mobilePlatform) mobileLastPageByTab.current[mobileTabForPage(next)] = next;
    setPage(next);
    if (mobilePlatform && typeof window !== "undefined") {
      const current = window.history.state;
      const state = {
        ...(current && typeof current === "object" ? current : {}),
        msimeSettings: true,
        page: next,
      } as Record<string, unknown>;
      delete state.panel;
      if (replace) window.history.replaceState(state, "");
      else window.history.pushState(state, "");
    }
    if (next === "community") setCommunityDestination("all");
  };

  useEffect(() => {
    if (!route || route.nonce === handledRoute.current) return;
    handledRoute.current = route.nonce;
    // A route to a page this host does not offer opens 输入 rather than an empty page.
    selectPage(requestedPage(route.page, availablePages, "input"));
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
        { ...(current && typeof current === "object" ? current : {}), msimeSettings: true, page },
        "",
      );
    }
    const onPopState = (event: PopStateEvent) => {
      const state = event.state;
      if (state?.msimeSettings === true && typeof state.page === "string") {
        const restored = requestedPage(state.page, availablePagesRef.current, "input");
        mobileLastPageByTab.current[mobileTabForPage(restored)] = restored;
        setPage(restored);
      }
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [mobilePlatform]);

  const selectMobileTab = (tab: SettingsPageId) => {
    if (!mobilePrimaryPageIds.includes(tab as MobilePrimaryPageId)) return;
    const primary = tab as MobilePrimaryPageId;
    if (primary === "account") setAccountLoginReturnPage(null);
    const remembered = mobileLastPageByTab.current[primary];
    const available = availablePages.some((item) => item.id === remembered);
    selectPage(available && !mobileHiddenPageIds.includes(remembered) ? remembered : primary);
  };

  const openAccountLogin = () => {
    if (mobilePlatform) setAccountLoginReturnPage(page);
    selectPage("account");
  };

  const finishAccountLogin = () => {
    const previous = accountLoginReturnPage;
    setAccountLoginReturnPage(null);
    if (!previous) return;
    mobileLastPageByTab.current[mobileTabForPage(previous)] = previous;
    setPage(previous);
    if (mobilePlatform && typeof window !== "undefined") window.history.back();
  };

  useEffect(() => {
    const pageAvailable =
      availablePages.some((item) => item.id === page) &&
      (!mobilePlatform || !mobileHiddenPageIds.includes(page));
    if (!pageAvailable) setPage(mobilePlatform && hasHomePage ? "home" : "input");
  }, [availablePages, hasHomePage, mobileHiddenPageIds, mobilePlatform, page, setPage]);

  return {
    mobileActiveTab,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
  };
}
