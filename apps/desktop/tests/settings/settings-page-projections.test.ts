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
    mobileHiddenPageIds: ["floating-toolbar"],
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
