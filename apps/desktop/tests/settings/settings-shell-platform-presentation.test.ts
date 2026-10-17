import { expect, test } from "vitest";

import { settingsShellPlatformPresentation } from "../../../../packages/ui/src/settings/settings-shell-platform-presentation";

const pages = [
  { id: "input", title: "输入" },
  { id: "help", title: "帮助与反馈" },
];

function presentation(
  overrides: Partial<Parameters<typeof settingsShellPlatformPresentation>[0]> = {},
) {
  return settingsShellPlatformPresentation({
    settingsPlatform: "win",
    mobilePlatform: false,
    mobileActiveTab: "home",
    page: "input",
    hasWindowControl: true,
    hasWindowDrag: false,
    availablePages: pages,
    untitledOnPhone: [],
    ...overrides,
  });
}

test("按宿主窗口能力区分标题栏搜索与侧栏搜索", () => {
  expect(presentation()).toMatchObject({
    winShell: true,
    macShell: false,
    linuxShell: false,
    ipadShell: false,
    titlebarShown: true,
    searchInTitlebar: true,
    searchInSidebar: false,
    sidebarBrandShown: false,
  });
  expect(presentation({ hasWindowControl: false })).toMatchObject({
    titlebarShown: false,
    searchInTitlebar: false,
    searchInSidebar: true,
    sidebarBrandShown: true,
  });
});

test("macOS、Linux 和 HarmonyOS 2-in-1 使用侧栏搜索", () => {
  expect(presentation({ settingsPlatform: "mac" })).toMatchObject({
    macShell: true,
    linuxShell: false,
    searchInTitlebar: false,
    searchInSidebar: true,
    sidebarBrandShown: true,
  });
  expect(presentation({ settingsPlatform: "linux" })).toMatchObject({
    macShell: false,
    linuxShell: true,
    searchInTitlebar: false,
    searchInSidebar: false,
  });
  expect(presentation({ settingsPlatform: "hm2" })).toMatchObject({
    harmonyLook: true,
    searchInSidebar: true,
  });
});

test("鸿蒙手机和触屏对话页计算推入、撑满和标题折叠状态", () => {
  expect(
    presentation({ settingsPlatform: "harmony", mobilePlatform: true, page: "help" }),
  ).toMatchObject({
    harmonyLook: true,
    harmonyPhone: true,
    tabRootPage: false,
    pushedPage: true,
    fillsContent: false,
    contentFills: false,
    collapsingTitle: true,
  });
  expect(
    presentation({ settingsPlatform: "harmony", mobilePlatform: true, page: "chat" }),
  ).toMatchObject({
    pushedPage: true,
    contentFills: true,
    collapsingTitle: true,
  });
  expect(
    presentation({ settingsPlatform: "harmony", mobilePlatform: true, page: "try-keyboard" }),
  ).toMatchObject({
    fillsContent: true,
    contentFills: true,
    collapsingTitle: false,
  });
});

test("触屏和 iPad 按标签页计算侧栏与页面标题", () => {
  expect(
    presentation({
      settingsPlatform: "ipad",
      mobilePlatform: true,
      mobileActiveTab: "home",
      page: "help",
      availablePages: pages,
    }),
  ).toMatchObject({ ipadSidebarShown: true, pageTitle: "帮助与反馈" });
  expect(
    presentation({
      settingsPlatform: "ipad",
      mobilePlatform: true,
      mobileActiveTab: "community",
      page: "help",
    }),
  ).toMatchObject({ ipadSidebarShown: false });
  expect(
    presentation({
      settingsPlatform: "android",
      mobilePlatform: true,
      page: "unknown",
      untitledOnPhone: ["unknown"],
    }),
  ).toMatchObject({ pageTitle: "输入", collapsingTitle: false });
});
