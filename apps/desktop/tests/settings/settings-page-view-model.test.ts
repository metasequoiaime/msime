import { expect, test } from "vitest";
import { settingsPageLinks, settingsPageTitle } from "@msime/ui";

const pages = [
  { id: "appearance" as const, title: "外观", icon: "appearance.svg" },
  { id: "about" as const, title: "关于", icon: "about.svg" },
];

test("resolves page titles with the appearance fallback", () => {
  expect(settingsPageTitle(pages, "about")).toBe("关于");
  expect(settingsPageTitle(pages, "feedback")).toBe("外观");
});

test("projects compact More settings links", () => {
  expect(settingsPageLinks(pages)).toEqual(pages);
});
