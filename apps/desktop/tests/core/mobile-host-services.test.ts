import { expect, test, vi } from "vitest";
import {
  createMobileHostServices,
  type MobileHostServiceDependencies,
} from "../../src/core/mobile-host-services";

function dependencies() {
  const invokeMock = vi.fn(async () => undefined);
  const listenMock = vi.fn(async () => vi.fn());
  const navigateVoice = vi.fn();
  const value: MobileHostServiceDependencies = {
    invoke: invokeMock as unknown as MobileHostServiceDependencies["invoke"],
    listen: listenMock as unknown as MobileHostServiceDependencies["listen"],
    navigateVoice,
  };
  return { value, invokeMock, listenMock, navigateVoice };
}

test("Android and iOS share Tauri account, community, AI and skin services", async () => {
  const androidDependencies = dependencies();
  const iosDependencies = dependencies();
  const android = createMobileHostServices("android", androidDependencies.value);
  const ios = createMobileHostServices("ios", iosDependencies.value);

  for (const services of [android, ios]) {
    expect(services.account).toBeDefined();
    expect(services.chat).toBeDefined();
    expect(services.aiAssistant).toBeDefined();
    expect(services.communitySkins).toBeDefined();
    expect(services.communityResources).toBeDefined();
    expect(services.aiSkins).toBeDefined();
    expect(services.customSkinLibrary).toBeDefined();
    expect(services.touchKeyboardSchemes).toBe(true);
    expect(services.customTouchKeyboardSkins).toBe(true);
    expect(services.mobileKeyboardFeedback).toBeDefined();
  }

  await android.communitySkins?.list(0, "杉", false, null);
  await ios.communitySkins?.list(0, "杉", true, "acg");
  expect(androidDependencies.invokeMock).toHaveBeenCalledWith("community_skin_list", {
    offset: 0,
    search: "杉",
    mine: false,
    category: null,
  });
  expect(iosDependencies.invokeMock).toHaveBeenCalledWith("community_skin_list", {
    offset: 0,
    search: "杉",
    mine: true,
    category: "acg",
  });

  // 发布带上分类，作者修改分类走单独的命令。
  const id = "10000000-0000-4000-8000-000000000001";
  await android.communitySkins?.publish(id, "晨雾", "", {} as never, "guofeng");
  expect(androidDependencies.invokeMock).toHaveBeenCalledWith("community_skin_publish", {
    id,
    name: "晨雾",
    description: "",
    design: {},
    category: "guofeng",
  });
  await ios.communitySkins?.setCategory(id, "food");
  expect(iosDependencies.invokeMock).toHaveBeenCalledWith("community_skin_set_category", {
    id,
    category: "food",
  });
});

test("mobile reports name the backend kind of each gallery", async () => {
  const mobile = dependencies();
  const services = createMobileHostServices("android", mobile.value);
  const id = "10000000-0000-4000-8000-000000000003";

  await services.communitySkins?.report?.(id, "色情低俗", "");
  await services.communityResources?.report?.("dictionary", id, "垃圾广告", "刷屏");
  await services.communityResources?.report?.("reply", id, "其他", "");

  expect(mobile.invokeMock.mock.calls).toEqual([
    ["community_report", { kind: "skins", id, reason: "色情低俗", detail: "" }],
    ["community_report", { kind: "dictionaries", id, reason: "垃圾广告", detail: "刷屏" }],
    ["community_report", { kind: "replies", id, reason: "其他", detail: "" }],
  ]);
});

test("platform-only mobile actions stay explicit", async () => {
  const androidDependencies = dependencies();
  const iosDependencies = dependencies();
  const android = createMobileHostServices("android", androidDependencies.value);
  const ios = createMobileHostServices("ios", iosDependencies.value);

  expect(android.account?.appleLogin).toBeUndefined();
  expect(android.openVoice).toBeUndefined();
  expect(android.home?.openKeyboard).toBeDefined();
  expect(android.home?.showInputMethodPicker).toBeDefined();
  expect(ios.account?.appleLogin).toBeDefined();
  expect(ios.openVoice).toBeDefined();
  expect(ios.home?.openKeyboard).toBeUndefined();
  expect(ios.home?.openSystemKeyboardSettings).toBeDefined();

  await android.openSystemKeyboardSettings?.();
  await ios.openSystemKeyboardSettings?.();
  await ios.home?.openSystemKeyboardSettings?.();
  await ios.account?.appleLogin?.();
  await ios.openVoice?.();

  expect(androidDependencies.invokeMock).toHaveBeenCalledWith("android_open_input_method_settings");
  expect(iosDependencies.invokeMock).toHaveBeenCalledWith("open_system_keyboard_settings");
  expect(iosDependencies.invokeMock).toHaveBeenCalledWith("account_apple_login");
  expect(iosDependencies.navigateVoice).toHaveBeenCalledOnce();
});
