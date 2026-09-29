import { expect, test } from "vitest";
import { settingsSidebarGroups } from "@msime/ui";

const pages = [
  { id: "input" },
  { id: "shortcuts" },
  { id: "account" },
  { id: "chat" },
  { id: "about" },
  { id: "more" },
];

test("filters mobile-only and hidden pages before grouping", () => {
  expect(
    settingsSidebarGroups(pages, {
      mobile: true,
      hiddenPageIds: ["shortcuts"],
      macos: false,
    }),
  ).toEqual([[{ id: "input" }, { id: "account" }, { id: "chat" }, { id: "about" }]]);
});

test("uses macOS groups and keeps unlisted pages visible", () => {
  expect(
    settingsSidebarGroups(pages, {
      mobile: false,
      hiddenPageIds: [],
      macos: true,
    }),
  ).toEqual([
    [{ id: "input" }, { id: "shortcuts" }],
    [{ id: "account" }],
    [{ id: "chat" }],
    [{ id: "about" }],
  ]);
});
