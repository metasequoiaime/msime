import { expect, test } from "vitest";
import {
  pages,
  settingsNavGroups,
  settingsPageAliases,
  subPageParents,
} from "../../../../packages/ui/src/settings/settings-page-registry";

test("keeps the current settings route registry in navigation order", () => {
  expect(pages.map((page) => page.id)).toEqual([
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
    "developer",
    "feedback",
    "about",
    "ai",
    "chat",
    "vocabulary",
    "help",
    "more",
  ]);
});

test("keeps navigation groups and nested route ownership aligned", () => {
  expect(settingsNavGroups).toEqual([
    { title: "打字", ids: ["input", "expression", "shortcuts", "dictionary"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "更多输入方式", ids: ["screen-keyboard", "voice", "handwriting"] },
    { title: "工具", ids: ["tools", "typing-statistics", "plugins"] },
    { title: "账户与社区", ids: ["account", "community"] },
    { title: "支持", ids: ["developer", "feedback", "about"] },
  ]);
  expect(subPageParents).toEqual({
    ai: "expression",
    chat: "expression",
    vocabulary: "dictionary",
    help: "feedback",
  });
  expect(settingsPageAliases).toEqual({ helpcode: "input", download: "about" });
});

test("lists every navigation page exactly once, and only pages the registry knows", () => {
  const ids = settingsNavGroups.flatMap((group) => group.ids);
  expect(new Set(ids).size).toBe(ids.length);
  const known = new Set<string>(pages.map((page) => page.id));
  for (const id of ids) expect(known.has(id)).toBe(true);
  // 旧页面 id 如果仍是一个页面，别名会把那一页挡住。
  for (const [alias, target] of Object.entries(settingsPageAliases)) {
    expect(known.has(alias)).toBe(false);
    expect(known.has(target)).toBe(true);
  }
});
