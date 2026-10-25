import { expect, test } from "vitest";
import {
  harmonyTabIcon,
  mobilePageTitle,
  mobileTabIcon,
  mobileTabTitle,
} from "../../../../packages/ui/src/settings/mobile-tab-helpers";
import { fluentIconPaths } from "../../../../packages/ui/src/core/fluent-icons";

test("maps the home tab icon and compact labels", () => {
  expect(mobileTabIcon("home", "msime.svg")).toMatch(/home\.svg$/);
  expect(harmonyTabIcon("home")).toBe("home");
  expect(fluentIconPaths.home).toBeDefined();
  expect(mobileTabIcon("community", "community.svg")).toBe("community.svg");
  expect(mobileTabTitle("home", "首页")).toBe("首页");
  expect(mobileTabTitle("typing-statistics", "打字统计")).toBe("统计");
  expect(mobileTabTitle("account", "账号与同步")).toBe("我的");
  expect(mobileTabTitle("community", "社区")).toBe("社区");
});

// 第二格是「社区」：HarmonyOS 画地球字形，与另外三格一样宽；遮罩图标沿用页面注册表里社区页的图标。皮肤页回到「设置」下面，标签栏上没有它，页名仍叫「皮肤」。
test("names the 社区 tab and draws the community glyph", () => {
  expect(harmonyTabIcon("community")).toBe("globe");
  expect(fluentIconPaths.globe).toBeDefined();
  expect(mobilePageTitle("skin", "主题")).toBe("皮肤");
  expect(harmonyTabIcon("skin")).toBeUndefined();
  // 「全部设置」标签已去掉，`more` 不再有标签名或图标。
  expect(mobileTabTitle("more", "更多")).toBe("更多");
  expect(harmonyTabIcon("more")).toBeUndefined();
});

test("uses touch friendly page titles where desktop labels differ", () => {
  expect(mobilePageTitle("appearance", "候选窗口")).toBe("候选栏");
  expect(mobilePageTitle("screen-keyboard", "屏幕键盘")).toBe("键盘");
  // 「首页」标签的根页以应用名作大标题，标签上仍写「首页」。
  expect(mobilePageTitle("home", "首页")).toBe("水杉输入法");
  expect(mobilePageTitle("account", "账号与同步")).toBe("我的");
  expect(mobilePageTitle("community", "社区")).toBe("社区");
});
