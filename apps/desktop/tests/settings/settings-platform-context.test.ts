import { expect, test } from "vitest";
import { settingsPlatformContext, type HostCapabilities } from "@msime/ui";

const host = (platform: HostCapabilities["platform"], mobile_settings?: boolean) =>
  ({ platform, mobile_settings }) as HostCapabilities;

test("derives release and diagnostics platforms from Android and iOS hosts", () => {
  expect(settingsPlatformContext(host("android"))).toMatchObject({
    releasePlatform: "android",
    diagnosticsFallbackPlatform: "android",
  });
  expect(settingsPlatformContext(host("ios"))).toMatchObject({
    releasePlatform: "ios",
    diagnosticsFallbackPlatform: "ios",
    accountPlatform: "ios",
  });
  expect(settingsPlatformContext(host("android")).accountPlatform).toBe("android");
});

test("uses Linux as the release fallback and desktop diagnostics elsewhere", () => {
  expect(settingsPlatformContext(host("linux"))).toMatchObject({
    linux: true,
    releasePlatform: "linux",
    diagnosticsFallbackPlatform: "desktop",
  });
  expect(settingsPlatformContext(host("windows"))).toMatchObject({
    releasePlatform: "windows",
    diagnosticsFallbackPlatform: "desktop",
    accountPlatform: undefined,
  });
  expect(settingsPlatformContext(host("harmony")).accountPlatform).toBe("harmony");
});
