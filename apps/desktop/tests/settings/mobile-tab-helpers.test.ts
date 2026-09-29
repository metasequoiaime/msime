import { expect, test } from "vitest";
import {
  mobilePageTitle,
  mobileTabIcon,
  mobileTabTitle,
} from "../../../../packages/ui/src/settings/mobile-tab-helpers";

test("maps the settings tab icon and compact labels", () => {
  expect(mobileTabIcon("home", "home.svg")).toMatch(/settings\.svg$/);
  expect(mobileTabIcon("community", "community.svg")).toBe("community.svg");
  expect(mobileTabTitle("home", "首页")).toBe("设置");
  expect(mobileTabTitle("typing-statistics", "打字统计")).toBe("统计");
  expect(mobileTabTitle("account", "账户与同步")).toBe("我的");
  expect(mobileTabTitle("community", "社区")).toBe("社区");
});

test("uses touch friendly page titles where desktop labels differ", () => {
  expect(mobilePageTitle("appearance", "候选窗口")).toBe("候选栏");
  expect(mobilePageTitle("screen-keyboard", "屏幕键盘")).toBe("键盘");
  expect(mobilePageTitle("shortcuts", "快捷键")).toBe("外接键盘快捷键");
  expect(mobilePageTitle("account", "账户与同步")).toBe("我的");
  expect(mobilePageTitle("community", "社区")).toBe("社区");
});
