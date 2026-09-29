import { expect, test } from "vitest";
import { platformCopy, type PlatformCopyContext } from "@msime/ui";

const context = (patch: Partial<PlatformCopyContext> = {}): PlatformCopyContext => ({
  android: false,
  linux: false,
  macos: false,
  harmony: false,
  ios: false,
  mobile: false,
  ...patch,
});

test("provides Android help and privacy copy", () => {
  const copy = platformCopy(context({ android: true, mobile: true }));

  expect(copy.helpIntro).toContain("Android 平台的中文输入法");
  expect(copy.quickStart).toContain("语言和输入法");
  expect(copy.networkDescription).toContain("系统语音识别服务");
  expect(copy.aboutDescription).toBe("为 Android 触屏输入体验打造的开放中文输入法。");
});

test("distinguishes HarmonyOS desktop and touch copy", () => {
  expect(platformCopy(context({ harmony: true, mobile: false })).aboutDescription).toBe(
    "为 HarmonyOS 2-in-1 桌面输入体验打造的开放中文输入法。",
  );
  expect(platformCopy(context({ harmony: true, mobile: true })).aboutDescription).toBe(
    "为 HarmonyOS 触屏输入体验打造的开放中文输入法。",
  );
});

test("uses Windows copy as the desktop fallback", () => {
  const copy = platformCopy(context());

  expect(copy.helpIntro).toContain("Windows 平台的中文输入法");
  expect(copy.quickStart).toContain("Win + Space");
  expect(copy.aboutDescription).toBe("为现代 Windows 桌面体验打造的开放中文输入法。");
});
