import { expect, test } from "vitest";
import {
  pages,
  settingsNavGroups,
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
    "ai",
    "account",
    "community",
    "developer",
    "feedback",
    "about",
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
    { title: "工具", ids: ["tools", "typing-statistics", "plugins", "ai"] },
    { title: "账号", ids: ["account"] },
    { title: "支持", ids: ["developer", "feedback", "about"] },
  ]);
  expect(subPageParents).toEqual({
    chat: "ai",
    vocabulary: "dictionary",
    help: "feedback",
  });
});

test("lists every navigation page exactly once, and only pages the registry knows", () => {
  const ids = settingsNavGroups.flatMap((group) => group.ids);
  expect(new Set(ids).size).toBe(ids.length);
  const known = new Set<string>(pages.map((page) => page.id));
  for (const id of ids) expect(known.has(id)).toBe(true);
});
