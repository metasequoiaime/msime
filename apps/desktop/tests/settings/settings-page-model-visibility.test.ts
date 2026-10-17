import { expect, test } from "vitest";

import {
  mobileHiddenPageIds,
  settingsPageModelVisibility,
} from "../../../../packages/ui/src/settings/settings-page-model-visibility";

function visibility(overrides: Partial<Parameters<typeof settingsPageModelVisibility>[0]> = {}) {
  return settingsPageModelVisibility({
    mobilePlatform: false,
    settingsPlatform: "android",
    hasHost: true,
    linuxPlatform: false,
    windowsPlatform: false,
    macosPlatform: false,
    showRestartInputMethod: false,
    hasMcpServerStatus: false,
    showHelpcodeShiftEntry: false,
    editionUsesHelpcode: true,
    ...overrides,
  });
}

test("辅助码入口按触屏宿主开关与方案能力显示", () => {
  expect(visibility()).toMatchObject({ showHelpcode: true });
  expect(visibility({ mobilePlatform: true })).toMatchObject({ showHelpcode: false });
  expect(visibility({ mobilePlatform: true, showHelpcodeShiftEntry: true })).toMatchObject({
    showHelpcode: true,
  });
  expect(visibility({ editionUsesHelpcode: false })).toMatchObject({ showHelpcode: false });
});

test("维护页在宿主服务或平台维护能力存在时显示", () => {
  expect(visibility()).toMatchObject({ showDeveloperPage: false });
  expect(visibility({ hasMcpServerStatus: true })).toMatchObject({ showDeveloperPage: true });
  expect(visibility({ showRestartInputMethod: true })).toMatchObject({
    showDeveloperPage: true,
  });
  expect(visibility({ linuxPlatform: true })).toMatchObject({ showDeveloperPage: true });
  expect(visibility({ settingsPlatform: "win" })).toMatchObject({ showDeveloperPage: false });
  expect(visibility({ hasHost: false })).toMatchObject({ showDeveloperPage: true });
});

test("鸿蒙手机保留大标题，其他触屏根标签页没有大标题", () => {
  expect(visibility({ settingsPlatform: "harmony" }).untitledOnPhone).toEqual([]);
  expect(visibility({ settingsPlatform: "android" }).untitledOnPhone).toEqual([
    "home",
    "typing-statistics",
    "account",
  ]);
  expect(visibility({ settingsPlatform: "ipad" }).untitledOnPhone).toEqual([
    "home",
    "typing-statistics",
    "account",
  ]);
});

test("手机隐藏页列表保持固定且不可变", () => {
  expect(mobileHiddenPageIds).toEqual(["shortcuts", "floating-toolbar", "plugins"]);
});
