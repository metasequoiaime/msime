import { expect, test } from "vitest";
import {
  desktopMergedPages,
  desktopPage,
  pages,
  settingsNavGroups,
  subPageParents,
} from "../../../../packages/ui/src/settings/settings-page-registry";

test("注册表保留已有页面与稳定路由", () => {
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
    "usage-reporting",
    "mcp",
    "privacy",
    "try-keyboard",
  ]);
});

test("keeps navigation groups and nested route ownership aligned", () => {
  expect(settingsNavGroups).toEqual([
    { title: "打字", ids: ["input", "dictionary", "shortcuts", "expression"] },
    { title: "输入方式", ids: ["voice", "screen-keyboard", "handwriting"] },
    { title: "外观", ids: ["skin", "appearance", "floating-toolbar"] },
    { title: "工具", ids: ["tools", "ai", "mcp", "plugins"] },
    { title: "数据", ids: ["typing-statistics", "account"] },
    { title: "支持", ids: ["developer", "feedback", "about"] },
  ]);
  expect(subPageParents).toEqual({
    chat: "ai",
    vocabulary: "dictionary",
    help: "feedback",
    "usage-reporting": "about",
    privacy: "account",
    "try-keyboard": "home",
  });
  // 桌面上并进别的页、只留作路由别名的页面。
  expect(desktopMergedPages).toEqual({ feedback: "about", mcp: "ai" });
  expect(desktopPage("feedback")).toBe("about");
  // MCP 在桌面上是「AI」页的「连接助手」标签。
  expect(desktopPage("mcp")).toBe("ai");
  expect(desktopPage("help")).toBe("help");
});

test("lists every navigation page exactly once, and only pages the registry knows", () => {
  const ids = settingsNavGroups.flatMap((group) => group.ids);
  expect(new Set(ids).size).toBe(ids.length);
  const known = new Set<string>(pages.map((page) => page.id));
  for (const id of ids) expect(known.has(id)).toBe(true);
});
