import { mobilePrimaryPageIds, type MobilePrimaryPageId } from "./settings-navigation-helpers";
import type { SettingsPlatform } from "../theme/platform-tokens";

export interface SettingsShellPlatformPresentationOptions {
  settingsPlatform: SettingsPlatform;
  mobilePlatform: boolean;
  mobileActiveTab: string;
  page: string;
  hasWindowControl: boolean;
  hasWindowDrag: boolean;
  availablePages: readonly { id: string; title: string }[];
  untitledOnPhone: readonly string[];
}

/** 设置外壳只依赖宿主与当前路由的展示判定，集中在一个无副作用函数里。 */
export function settingsShellPlatformPresentation({
  settingsPlatform,
  mobilePlatform,
  mobileActiveTab,
  page,
  hasWindowControl,
  hasWindowDrag,
  availablePages,
  untitledOnPhone,
}: SettingsShellPlatformPresentationOptions) {
  const harmonyLook = settingsPlatform === "harmony" || settingsPlatform === "hm2";
  const harmonyThemePlatform = harmonyLook ? settingsPlatform : undefined;
  const harmonyPhone = settingsPlatform === "harmony" && mobilePlatform;
  const tabRootPage = mobilePrimaryPageIds.includes(page as MobilePrimaryPageId);
  const pushedPage = harmonyPhone && !tabRootPage;
  const fillsContent = mobilePlatform && page === "try-keyboard";
  const contentFills = fillsContent || (mobilePlatform && page === "chat");
  const winShell = settingsPlatform === "win";
  const macShell = settingsPlatform === "mac";
  const linuxShell = settingsPlatform === "linux";
  const ipadShell = settingsPlatform === "ipad";
  const ipadSidebarShown = mobileActiveTab === "home";
  const titlebarShown = !mobilePlatform && !macShell && Boolean(hasWindowControl || hasWindowDrag);
  const pageTitle = availablePages.find((item) => item.id === page)?.title ?? "输入";
  const collapsingTitle =
    mobilePlatform &&
    mobileActiveTab === "home" &&
    !untitledOnPhone.includes(page) &&
    !fillsContent;
  const searchInTitlebar = winShell && titlebarShown;
  const searchInSidebar =
    macShell || settingsPlatform === "hm2" || ipadShell || (winShell && !titlebarShown);
  const sidebarBrandShown = !mobilePlatform && !titlebarShown;

  return {
    harmonyLook,
    harmonyThemePlatform,
    harmonyPhone,
    tabRootPage,
    pushedPage,
    fillsContent,
    contentFills,
    winShell,
    macShell,
    linuxShell,
    ipadShell,
    ipadSidebarShown,
    titlebarShown,
    pageTitle,
    collapsingTitle,
    searchInTitlebar,
    searchInSidebar,
    sidebarBrandShown,
  } as const;
}
