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
    "input",
    "expression",
    "shortcuts",
    "dictionary",
    "skin",
    "appearance",
    "floating-toolbar",
    "screen-keyboard",
    "voice",
    "handwriting",
    "tools",
    "typing-statistics",
    "plugins",
    "account",
    "community",
    "feedback",
    "about",
    "ai",
    "chat",
    "vocabulary",
    "help",
  ]);
  // 「维护与诊断」只在宿主声明 developer 时出现；不传这一项的旧调用方看到的页面不变。
  expect(
    settingsPageCatalog({
      ...allCapabilities,
      developer: true,
      mobile: false,
      modeSwitchShortcuts: true,
      panelShortcuts: true,
      desktopMaintenanceShortcuts: true,
      helpcodeShiftEntry: true,
      android: false,
      harmony: false,
      macos: false,
    }).availablePages.map((page) => page.id),
  ).toContain("developer");
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
  // 辅助码现在是输入页的一组，不再是单独的页面；输入页在两种宿主上都列在「全部设置」里。
  expect(android.mobileSecondaryPages.map((page) => page.id)).toContain("input");
  expect(harmonyTwoInOne.mobileSecondaryPages.map((page) => page.id)).toContain("input");
  expect(harmonyTwoInOne.mobileSecondaryPages.map((page) => page.id)).not.toContain("helpcode");
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
