import type { HostCapabilities, HostPlatform } from "../index";

export interface SettingsPlatformContext {
  host?: HostCapabilities;
  linux: boolean;
  android: boolean;
  ios: boolean;
  harmony: boolean;
  mobile: boolean;
  windows: boolean;
  macos: boolean;
  releasePlatform: HostPlatform | null;
  diagnosticsFallbackPlatform: "android" | "ios" | "desktop";
  accountPlatform: "android" | "ios" | "harmony" | undefined;
}

/** Derives the host and form-factor flags shared by the settings page. */
export function settingsPlatformContext(host?: HostCapabilities): SettingsPlatformContext {
  const android = host?.platform === "android";
  const ios = host?.platform === "ios";
  const harmony = host?.platform === "harmony";
  const mobile = host ? host.mobile_settings : false;
  const linux = host?.platform === "linux";
  return {
    host,
    linux,
    android,
    ios,
    harmony,
    mobile,
    windows: host?.platform === "windows",
    macos: host?.platform === "macos",
    releasePlatform: host?.platform ?? null,
    diagnosticsFallbackPlatform: android ? "android" : ios ? "ios" : "desktop",
    accountPlatform: android ? "android" : ios ? "ios" : harmony ? "harmony" : undefined,
  };
}
