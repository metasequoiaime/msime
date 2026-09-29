import { expect, test } from "vitest";
import {
  mobilePrimaryPageIds,
  mobileTabForPage,
  requestedPage,
} from "../../../../packages/ui/src/settings/settings-navigation-helpers";

const pages = [{ id: "appearance" }, { id: "input" }, { id: "home" }] as const;

test("maps primary and nested pages to the mobile tab stack", () => {
  expect(mobilePrimaryPageIds).toEqual(["home", "community", "typing-statistics", "account"]);
  expect(mobileTabForPage("community")).toBe("community");
  expect(mobileTabForPage("input")).toBe("home");
});

test("resolves aliases before known pages and falls back for unknown routes", () => {
  const aliases = { helpcode: "input" as const };
  expect(requestedPage("helpcode", pages, aliases, "appearance")).toBe("input");
  expect(requestedPage("home", pages, aliases, "appearance")).toBe("home");
  expect(requestedPage("missing", pages, aliases, "appearance")).toBe("appearance");
  expect(requestedPage(undefined, pages, aliases, "appearance")).toBe("appearance");
});
