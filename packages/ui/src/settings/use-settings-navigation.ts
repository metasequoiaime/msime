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
  type SettingsPageId,
} from "./mobile-navigation";

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

  const selectPage = (next: SettingsPageId) => {
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
      window.history.pushState(state, "");
    }
    if (next === "community") setCommunityDestination("all");
  };

  useEffect(() => {
    if (!route || route.nonce === handledRoute.current) return;
    handledRoute.current = route.nonce;
    selectPage(requestedPage(route.page));
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
        const restored = requestedPage(state.page);
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
    if (!pageAvailable) setPage(mobilePlatform && hasHomePage ? "home" : "appearance");
  }, [availablePages, hasHomePage, mobileHiddenPageIds, mobilePlatform, page, setPage]);

  return {
    mobileActiveTab,
    selectPage,
    selectMobileTab,
    openAccountLogin,
    finishAccountLogin,
  };
}
