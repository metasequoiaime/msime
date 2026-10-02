import { availableSettingsPages, type AvailablePageCapabilities } from "./available-pages";
import { mobileHiddenPageIds as getMobileHiddenPageIds } from "./mobile-hidden-pages";
import { splitMobilePages } from "./mobile-navigation";
import type { SettingsPageId } from "./settings-page-registry";
import { settingsSidebarGroups, type SettingsSidebarGroupsOptions } from "./sidebar-groups";

export interface SettingsPageCatalogOptions extends AvailablePageCapabilities {
  modeSwitchShortcuts: boolean;
  panelShortcuts: boolean;
  desktopMaintenanceShortcuts: boolean;
  macos: SettingsSidebarGroupsOptions["macos"];
}

/** 按宿主能力组装页面列表及其桌面、手机投影。页面和分组都取自 `settings-page-registry`，与共享设置外壳一致。 */
export function settingsPageCatalog(options: SettingsPageCatalogOptions) {
  const availablePages = availableSettingsPages({
    home: options.home,
    typingStatistics: options.typingStatistics,
    vocabularyReview: options.vocabularyReview,
    account: options.account,
    chat: options.chat,
    community: options.community,
    floatingToolbar: options.floatingToolbar,
    plugins: options.plugins,
    developer: options.developer,
    mobile: options.mobile,
  });
  // Physical-keyboard shortcuts and a desktop floating toolbar have no phone
  // surface. HarmonyOS keeps those controls in the input-method panel on a 2-in-1,
  // but its phone panel is still a touch keyboard, so the settings entry must not
  // leak the PC key descriptions into the phone's "全部设置" list.
  //
  // The shortcuts page itself follows the projected capabilities rather than a
  // platform name so a mobile device with a physical keyboard can still reach it.
  const mobileHiddenPageIds: readonly SettingsPageId[] = getMobileHiddenPageIds({
    modeSwitchShortcuts: options.modeSwitchShortcuts,
    panelShortcuts: options.panelShortcuts,
    desktopMaintenanceShortcuts: options.desktopMaintenanceShortcuts,
  });
  const sidebarGroups = settingsSidebarGroups(availablePages, {
    mobile: options.mobile,
    hiddenPageIds: mobileHiddenPageIds,
    macos: options.macos,
  });
  // Keep the mobile bar in source tab order. All other visible pages are reached
  // from the keyboard tab's settings list, and hidden pages stay out of that list.
  const { primary: mobilePrimaryPages, secondary: mobileSecondaryPages } = splitMobilePages(
    availablePages,
    mobileHiddenPageIds,
  );

  return {
    availablePages,
    mobileHiddenPageIds,
    sidebarGroups,
    mobilePrimaryPages,
    mobileSecondaryPages,
  };
}

export type SettingsPageCatalog = ReturnType<typeof settingsPageCatalog>;
