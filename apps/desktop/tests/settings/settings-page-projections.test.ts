import { expect, test } from "vitest";
import { mobilePageTitle } from "../../../../packages/ui/src/settings/mobile-tab-helpers";
import { settingsPageProjections } from "../../../../packages/ui/src/settings/settings-page-projections";

const project = (overrides: Partial<Parameters<typeof settingsPageProjections>[0]> = {}) =>
  settingsPageProjections({
    mobilePlatform: false,
    hasHomePage: true,
    hasTypingStatistics: true,
    hasVocabularyReview: true,
    hasAccount: true,
    hasChat: true,
    hasCommunity: true,
    showFloatingToolbar: true,
    showDeveloperPage: true,
    hasPlugins: true,
    mobileHiddenPageIds: ["floating-toolbar", "plugins"],
    mobilePageTitle,
    ...overrides,
  });

test("filters pages by host capabilities while preserving registry order", () => {
  const desktop = project({
    hasHomePage: false,
    hasChat: false,
    hasCommunity: false,
    showFloatingToolbar: false,
  });

  expect(desktop.availablePages.map((page) => page.id)).not.toContain("home");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("chat");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("community");
  expect(desktop.availablePages.map((page) => page.id)).not.toContain("floating-toolbar");
  expect(desktop.availablePages.map((page) => page.id)).toEqual(
    expect.arrayContaining(["appearance", "input"]),
  );
});

test("projects mobile pages into tabs, grouped settings, and sidebar sections", () => {
  const mobile = project({
    mobilePlatform: true,
    mobileHiddenPageIds: ["shortcuts", "floating-toolbar"],
  });

  expect(mobile.availablePages.find((page) => page.id === "appearance")?.title).toBe("候选栏");
  expect(mobile.availablePages.map((page) => page.id)).not.toContain("download");
  expect(mobile.mobilePrimaryPages.map((page) => page.id)).toEqual([
    "home",
    "community",
    "typing-statistics",
    "account",
  ]);
  expect(mobile.mobileSecondaryGroups.flat().map((page) => page.id)).not.toEqual(
    expect.arrayContaining([
      "home",
      "community",
      "shortcuts",
      "floating-toolbar",
      "about",
      "feedback",
    ]),
  );
  expect(mobile.sidebarGroups[0].map((page) => page.id)).toEqual(["home"]);
  expect(mobile.sidebarGroups.flat().map((page) => page.id)).not.toContain("more");
});

test("places the 插件 page beside 云剪贴板 and drops it where the host does not back it", () => {
  const desktop = project();
  const group = desktop.sidebarGroups.find((ids) => ids.some((page) => page.id === "tools"));
  expect(group?.map((page) => page.id)).toEqual([
    "account",
    "tools",
    "plugins",
    "typing-statistics",
    "community",
    "download",
  ]);
  expect(desktop.availablePages.find((page) => page.id === "plugins")?.title).toBe("插件");

  expect(project({ hasPlugins: false }).availablePages.map((page) => page.id)).not.toContain(
    "plugins",
  );
  const phone = project({ mobilePlatform: true });
  expect(phone.sidebarGroups.flat().map((page) => page.id)).not.toContain("plugins");
  expect(phone.mobileSecondaryGroups.flat().map((page) => page.id)).not.toContain("plugins");
});
