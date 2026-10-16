import { expect, test } from "vitest";
import {
  harmonyTabIcon,
  mobilePageTitle,
  mobileTabIcon,
  mobileTabTitle,
} from "../../../../packages/ui/src/settings/mobile-tab-helpers";
import { fluentIconPaths } from "../../../../packages/ui/src/core/fluent-icons";

test("maps the settings tab icon and compact labels", () => {
  expect(mobileTabIcon("home", "home.svg")).toMatch(/settings\.svg$/);
  expect(mobileTabIcon("community", "community.svg")).toBe("community.svg");
  expect(mobileTabTitle("home", "首页")).toBe("设置");
  expect(mobileTabTitle("typing-statistics", "打字统计")).toBe("统计");
  expect(mobileTabTitle("account", "账号与同步")).toBe("我的");
  expect(mobileTabTitle("community", "社区")).toBe("社区");
});

// 「对话」标签页叫「对话」，画对话气泡：页面注册表给 `chat` 的图标是问号，放在标签栏上会被看成「帮助」。
test("names the 全部设置 tab and draws it as a grid", () => {
  expect(mobileTabTitle("more", "更多")).toBe("全部设置");
  expect(harmonyTabIcon("more")).toBe("grid");
  expect(fluentIconPaths.chat).toMatch(/^M10 2a8 8 0 1 1/);
  expect(harmonyTabIcon("community")).toBeUndefined();
});

test("uses touch friendly page titles where desktop labels differ", () => {
  expect(mobilePageTitle("appearance", "候选窗口")).toBe("候选栏");
  expect(mobilePageTitle("screen-keyboard", "屏幕键盘")).toBe("键盘");
  expect(mobilePageTitle("account", "账号与同步")).toBe("我的");
  expect(mobilePageTitle("community", "社区")).toBe("社区");
});
