import { expect, test } from "vitest";
import { settingsPageCatalog } from "@msime/ui";

const allCapabilities = {
  home: true,
  typingStatistics: true,
  vocabularyReview: true,
  account: true,
  chat: true,
  community: true,
  floatingToolbar: true,
  plugins: true,
};

test("builds the complete desktop page catalog in registry order", () => {
  const catalog = settingsPageCatalog({
    ...allCapabilities,
    mobile: false,
    modeSwitchShortcuts: true,
    panelShortcuts: true,
    desktopMaintenanceShortcuts: true,
    helpcodeShiftEntry: true,
    android: false,
    harmony: false,
    macos: false,
  });

  expect(catalog.availablePages.map((page) => page.id)).toEqual([
    "home",
    "account",
    "chat",
    "community",
    "typing-statistics",
    "appearance",
    "input",
    "helpcode",
    "shortcuts",
    "dictionary",
    "vocabulary",
    "skin",
    "voice",
    "screen-keyboard",
    "handwriting",
    "tools",
    "plugins",
    "ai",
    "floating-toolbar",
    "help",
    "about",
    "feedback",
  ]);
  expect(catalog.mobileHiddenPageIds).toEqual(["floating-toolbar", "plugins"]);
  expect(catalog.mobilePrimaryPages.map((page) => page.id)).toEqual([
    "home",
    "community",
    "typing-statistics",
    "account",
  ]);
});

test("hides hardware shortcuts and floating toolbar on touch-only mobile hosts", () => {
  const catalog = settingsPageCatalog({
    ...allCapabilities,
    mobile: true,
    modeSwitchShortcuts: false,
    panelShortcuts: false,
    desktopMaintenanceShortcuts: false,
    helpcodeShiftEntry: false,
    android: false,
    harmony: false,
    macos: false,
  });

  expect(catalog.mobileHiddenPageIds).toEqual([
    "shortcuts",
    "floating-toolbar",
    "plugins",
    "helpcode",
  ]);
  expect(catalog.sidebarGroups.flat().map((page) => page.id)).not.toContain("shortcuts");
  expect(catalog.sidebarGroups.flat().map((page) => page.id)).not.toContain("floating-toolbar");
  expect(catalog.sidebarGroups.flat().map((page) => page.id)).not.toContain("plugins");
  expect(catalog.mobileSecondaryPages.map((page) => page.id)).not.toContain("helpcode");
  expect(catalog.mobileSecondaryPages.map((page) => page.id)).not.toContain("plugins");
});

test("keeps helper codes on Android and HarmonyOS while exposing keyboard shortcuts by capability", () => {
  const android = settingsPageCatalog({
    ...allCapabilities,
    mobile: true,
    modeSwitchShortcuts: false,
    panelShortcuts: false,
    desktopMaintenanceShortcuts: false,
    helpcodeShiftEntry: false,
    android: true,
    harmony: false,
    macos: false,
  });
  const harmonyTwoInOne = settingsPageCatalog({
    ...allCapabilities,
    mobile: true,
    modeSwitchShortcuts: true,
    panelShortcuts: false,
    desktopMaintenanceShortcuts: false,
    helpcodeShiftEntry: false,
    android: false,
    harmony: true,
    macos: false,
  });

  expect(android.mobileHiddenPageIds).toEqual(["shortcuts", "floating-toolbar", "plugins"]);
  expect(harmonyTwoInOne.mobileHiddenPageIds).toEqual(["floating-toolbar", "plugins"]);
  expect(harmonyTwoInOne.mobileSecondaryPages.map((page) => page.id)).toContain("helpcode");
  expect(harmonyTwoInOne.mobileSecondaryPages.map((page) => page.id)).toContain("shortcuts");
});

test("offers the 插件 page only where the host backs it", () => {
  const catalog = settingsPageCatalog({
    ...allCapabilities,
    plugins: false,
    mobile: false,
    modeSwitchShortcuts: true,
    panelShortcuts: true,
    desktopMaintenanceShortcuts: true,
    helpcodeShiftEntry: true,
    android: false,
    harmony: false,
    macos: true,
  });

  expect(catalog.availablePages.map((page) => page.id)).not.toContain("plugins");
});
