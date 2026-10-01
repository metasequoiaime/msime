import { expect, test } from "vitest";
import { settingsSidebarGroups } from "@msime/ui";
import { settingsNavGroups } from "../../../../packages/ui/src/settings/settings-page-registry";

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

test("derives the macOS groups from the shared navigation registry", () => {
  const ids = [...settingsNavGroups.flatMap((group) => group.ids), "help"].map((id) => ({ id }));
  const groups = settingsSidebarGroups(ids, { mobile: false, hiddenPageIds: [], macos: true });
  // 注册表里的分组原样保留；不在导航里的子页（帮助）仍按旧规则排在最后一组之前，不会消失。
  expect(groups.map((group) => group.map((page) => page.id))).toEqual([
    ...settingsNavGroups.slice(0, -1).map((group) => [...group.ids]),
    ["help"],
    [...settingsNavGroups[settingsNavGroups.length - 1].ids],
  ]);
  expect(groups.flat().map((page) => page.id)).not.toContain("helpcode");
});
