import { expect, test } from "vitest";
import { initialMobileTabPages, initialSettingsPage } from "@msime/ui";

test("explicit initial page takes precedence over mobile history", () => {
  expect(
    initialSettingsPage({
      initialPage: "voice",
      mobilePlatform: true,
      mobileHistoryState: { msimeSettings: true, page: "input" },
      hasHomePage: true,
    }),
  ).toBe("voice");
});

test("a marked mobile history entry restores its page", () => {
  expect(
    initialSettingsPage({
      mobilePlatform: true,
      mobileHistoryState: { msimeSettings: true, page: "dictionary" },
      hasHomePage: true,
    }),
  ).toBe("dictionary");
  expect(
    initialSettingsPage({
      mobilePlatform: true,
      mobileHistoryState: { msimeSettings: false, page: "dictionary" },
      hasHomePage: true,
    }),
  ).toBe("home");
});

test("unknown routes retain the appearance fallback", () => {
  expect(
    initialSettingsPage({ initialPage: "unknown", mobilePlatform: true, hasHomePage: true }),
  ).toBe("appearance");
  expect(initialSettingsPage({ mobilePlatform: false, hasHomePage: false })).toBe("appearance");
});

test("the current leaf initializes only its owning mobile tab", () => {
  expect(initialMobileTabPages("input")).toEqual({
    home: "input",
    community: "community",
    "typing-statistics": "typing-statistics",
    account: "account",
  });
  expect(initialMobileTabPages("community")).toEqual({
    home: "home",
    community: "community",
    "typing-statistics": "typing-statistics",
    account: "account",
  });
});
