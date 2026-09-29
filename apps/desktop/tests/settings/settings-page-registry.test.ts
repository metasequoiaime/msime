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
    "skin",
    "appearance",
    "floating-toolbar",
    "input",
    "expression",
    "shortcuts",
    "dictionary",
    "screen-keyboard",
    "voice",
    "handwriting",
    "account",
    "tools",
    "typing-statistics",
    "community",
    "download",
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
    ["skin", "appearance", "floating-toolbar"],
    ["input", "expression", "shortcuts", "dictionary"],
    ["screen-keyboard", "voice", "handwriting"],
    ["account", "tools", "typing-statistics", "community", "download"],
    ["developer", "feedback", "about"],
  ]);
  expect(subPageParents).toEqual({
    ai: "expression",
    chat: "expression",
    vocabulary: "dictionary",
    help: "feedback",
  });
  expect(settingsPageAliases).toEqual({ helpcode: "input" });
});
