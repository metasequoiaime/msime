import type { HostCapabilities } from "../index";
import { isLinuxDesktop } from "./platform-helpers";

export interface SettingsPlatformContext {
  host?: HostCapabilities;
  linux: boolean;
  android: boolean;
  ios: boolean;
  harmony: boolean;
  mobile: boolean;
  windows: boolean;
  macos: boolean;
}

/** Derives the host and form-factor flags shared by the settings page. */
export function settingsPlatformContext(host?: HostCapabilities): SettingsPlatformContext {
  const android = host?.platform === "android";
  const ios = host?.platform === "ios";
  const harmony = host?.platform === "harmony";
  const mobile = host?.mobile_settings ?? (ios || android || harmony);
  return {
    host,
    linux: host ? host.platform === "linux" : isLinuxDesktop(),
    android,
    ios,
    harmony,
    mobile,
    windows: host?.platform === "windows",
    macos: host?.platform === "macos",
  };
}
