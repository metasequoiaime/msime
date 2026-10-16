import { expect, test } from "vitest";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
} from "../../../../packages/ui/src/settings/settings-navigation-helpers";
import * as mobileNavigation from "../../../../packages/ui/src/settings/mobile-navigation";

const pages = [{ id: "appearance" }, { id: "input" }, { id: "home" }] as const;

test("maps primary and nested pages to the mobile tab stack", () => {
  // 第二格是「社区」，「全部设置」与「设置」重复，已去掉。
  expect(mobilePrimaryPageIds).toEqual(["home", "community", "typing-statistics", "account"]);
  expect(mobileTabForPage("chat")).toBe("home");
  expect(mobileTabForPage("typing-statistics")).toBe("typing-statistics");
  expect(mobileTabForPage("input")).toBe("home");
  // 社区页是「社区」标签页的根页；皮肤、词库、AI 辅助页（及其社区图库子视图）在「设置」标签页的栈上。
  expect(mobileTabForPage("community")).toBe("community");
  expect(mobileTabForPage("skin")).toBe("home");
  expect(mobileTabForPage("dictionary")).toBe("home");
  expect(mobileTabForPage("ai")).toBe("home");
  // 旧版本留在历史记录里的「全部设置」不再是标签页，回到「设置」。
  expect(mobileTabForPage("more")).toBe("home");
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
