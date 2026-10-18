import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { AccountCommunityDestination } from "../account/account-model";
import {
  mobileTabForPage,
  requestedPage,
  type MobilePrimaryPageId,
} from "./settings-navigation-helpers";
import { desktopPage, pages, type SettingsPageId } from "./settings-page-registry";
import { useSettingsContentScrollReset } from "./use-settings-content-scroll-reset";
import type { DictionaryTab } from "./dictionary-tabs";
import type { AboutTab } from "./about-tabs";

export interface UseSettingsPageRoutingOptions {
  initialPage?: string;
  mobilePlatform: boolean;
  hasHomePage: boolean;
}

/** 设置页共享路由状态：保留移动端标签页栈、桌面合并页映射和登录返回页。 */
export function useSettingsPageRouting({
  initialPage,
  mobilePlatform,
  hasHomePage,
}: UseSettingsPageRoutingOptions) {
  const restoredMobilePage =
    mobilePlatform &&
    typeof window !== "undefined" &&
    window.history.state?.msimeSettings === true &&
    typeof window.history.state.page === "string"
      ? window.history.state.page
      : undefined;
  const [requestedSettingsPage, setRequestedPage] = useState<SettingsPageId>(() =>
    requestedPage(
      initialPage ?? restoredMobilePage ?? (hasHomePage ? "home" : undefined),
      pages,
      "input",
    ),
  );
  const [aboutTab, setAboutTab] = useState<AboutTab>(
    requestedSettingsPage === "feedback" ? "feedback" : "version",
  );
  const setPage = useCallback<Dispatch<SetStateAction<SettingsPageId>>>(
    (next) => {
      // 同一设置窗口再次收到反馈路由时，也要从当前的关于标签切到反馈。
      if (!mobilePlatform && next === "feedback") setAboutTab("feedback");
      setRequestedPage(next);
    },
    [mobilePlatform],
  );
  // 桌面上并进别的页的页面照样可以路由打开，显示的是承载它的那一页。
  const page = mobilePlatform ? requestedSettingsPage : desktopPage(requestedSettingsPage);
  const [accountLoginReturnPage, setAccountLoginReturnPage] = useState<SettingsPageId | null>(null);
  // 每个底部标签页各有一个导航栈；共享页面为每个标签页记住最近显示的页面。
  const mobileInitialTab = mobileTabForPage(page);
  const mobileLastPageByTab = useRef<Record<MobilePrimaryPageId, SettingsPageId>>({
    home: mobileInitialTab === "home" ? page : "home",
    community: mobileInitialTab === "community" ? page : "community",
    "typing-statistics": mobileInitialTab === "typing-statistics" ? page : "typing-statistics",
    account: mobileInitialTab === "account" ? page : "account",
  });
  const settingsContentRef = useSettingsContentScrollReset(
    page === "about" && !mobilePlatform ? `about:${aboutTab}` : page,
  );
  const [communityDestination, setCommunityDestination] = useState<
    AccountCommunityDestination | "all"
  >("all");
  const [dictionaryTab, setDictionaryTab] = useState<DictionaryTab>("lookup");

  return {
    requestedSettingsPage,
    setPage,
    page,
    accountLoginReturnPage,
    setAccountLoginReturnPage,
    mobileLastPageByTab,
    settingsContentRef,
    communityDestination,
    setCommunityDestination,
    dictionaryTab,
    setDictionaryTab,
    aboutTab,
    setAboutTab,
  };
}
