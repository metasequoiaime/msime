import { expect, test } from "vitest";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
} from "../../../../packages/ui/src/settings/settings-navigation-helpers";
import * as mobileNavigation from "../../../../packages/ui/src/settings/mobile-navigation";

const pages = [{ id: "appearance" }, { id: "input" }, { id: "home" }] as const;

test("maps primary and nested pages to the mobile tab stack", () => {
  expect(mobilePrimaryPageIds).toEqual(["home", "community", "typing-statistics", "account"]);
  expect(mobileTabForPage("community")).toBe("community");
  expect(mobileTabForPage("input")).toBe("home");
  // 「关于」和它的二级页面「发送哪些内容」都留在「我的」。
  expect(mobileTabForPage("about")).toBe("account");
  expect(mobileTabForPage("usage-reporting")).toBe("account");
});

test("mobile navigation reuses the shared tab mapping", () => {
  expect(mobileNavigation.mobilePrimaryPageIds).toBe(mobilePrimaryPageIds);
  expect(mobileNavigation.mobileTabForPage).toBe(mobileTabForPage);
});

test("resolves known pages and falls back for routes outside the list", () => {
  expect(requestedPage("home", pages, "appearance")).toBe("home");
  expect(requestedPage("helpcode", pages, "appearance")).toBe("appearance");
  expect(requestedPage("missing", pages, "appearance")).toBe("appearance");
  expect(requestedPage(undefined, pages, "appearance")).toBe("appearance");
});
