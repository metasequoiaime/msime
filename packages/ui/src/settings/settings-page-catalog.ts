import { availableSettingsPages, type AvailablePageCapabilities } from "./available-pages";
import { mobileHiddenPageIds as getMobileHiddenPageIds } from "./mobile-hidden-pages";
import { splitMobilePages, type SettingsPageId } from "./mobile-navigation";
import { settingsSidebarGroups, type SettingsSidebarGroupsOptions } from "./sidebar-groups";

export interface SettingsPageCatalogOptions extends AvailablePageCapabilities {
  modeSwitchShortcuts: boolean;
  panelShortcuts: boolean;
  desktopMaintenanceShortcuts: boolean;
  helpcodeShiftEntry: boolean;
  android: boolean;
  harmony: boolean;
  macos: SettingsSidebarGroupsOptions["macos"];
}

/** Assembles the host-aware page registry and the desktop/mobile projections of that registry. */
export function settingsPageCatalog(options: SettingsPageCatalogOptions) {
  const availablePages = availableSettingsPages({
    home: options.home,
    typingStatistics: options.typingStatistics,
    vocabularyReview: options.vocabularyReview,
    account: options.account,
    chat: options.chat,
    community: options.community,
    floatingToolbar: options.floatingToolbar,
    mobile: options.mobile,
  });
  // Physical-keyboard shortcuts and a desktop floating toolbar have no phone
  // surface. HarmonyOS keeps those controls in the input-method panel on a 2-in-1,
  // but its phone panel is still a touch keyboard, so the settings entry must not
  // leak the PC key descriptions into the phone's "全部设置" list.
  //
  // Helper codes are per-host rather than per-form-factor. Android and iOS route
  // Shift helper codes through the engine, and HarmonyOS uses the same policy, so
  // those hosts keep the page even when their other hardware shortcuts are hidden.
  // The shortcuts page itself follows the projected capabilities rather than a
  // platform name so a mobile device with a physical keyboard can still reach it.
  const mobileHiddenPageIds: readonly SettingsPageId[] = getMobileHiddenPageIds({
    modeSwitchShortcuts: options.modeSwitchShortcuts,
    panelShortcuts: options.panelShortcuts,
    desktopMaintenanceShortcuts: options.desktopMaintenanceShortcuts,
    helpcodeShiftEntry: options.helpcodeShiftEntry,
    android: options.android,
    harmony: options.harmony,
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
