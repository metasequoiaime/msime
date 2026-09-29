import type { SettingsClient } from "../index";
import { settingsCapabilities } from "./settings-capabilities";
import { settingsPlatformContext } from "./settings-platform-context";

/** Combines host platform context and capability projections used by SettingsPage. */
export function settingsPageEnvironment(client: SettingsClient) {
  const platform = settingsPlatformContext(client.host);
  const capabilities = settingsCapabilities({
    host: platform.host,
    linux: platform.linux,
    android: platform.android,
    ios: platform.ios,
    harmony: platform.harmony,
    windows: platform.windows,
    macos: platform.macos,
    mobile: platform.mobile,
    canRestartInputMethod: Boolean(client.restartInputMethod),
    canInstallInputSource: Boolean(client.installInputSource),
    canListVoiceCaptureDevices: Boolean(client.listVoiceCaptureDevices),
  });
  return { ...platform, ...capabilities } as const;
}
