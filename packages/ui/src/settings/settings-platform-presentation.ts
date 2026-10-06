import { platformCopy, type PlatformCopyContext } from "./platform-copy";
import { platformResourceUrls } from "./platform-resource-urls";
import { voiceCaptureBackendOptions } from "./voice-capture-backend-options";
import { fullwidthShortcutChord } from "./platform-shortcuts";

export interface SettingsPlatformPresentationOptions extends PlatformCopyContext {
  clientHostedPlatform: boolean;
  windows: boolean;
}

/** Derives all platform-dependent labels, URLs, chords, and voice choices for settings. */
export function settingsPlatformPresentation({
  android,
  linux,
  macos,
  harmony,
  ios,
  mobile,
  windows,
  clientHostedPlatform,
}: SettingsPlatformPresentationOptions) {
  const { releasesPageUrl, licenseUrl, issuesUrl, privacyUrl } = platformResourceUrls({
    clientHostedPlatform,
    linux,
  });
  return {
    fullwidthChord: fullwidthShortcutChord(macos),
    captureBackendOptions: voiceCaptureBackendOptions({ linux, macos, windows, harmony }),
    releasesPageUrl,
    licenseUrl,
    issuesUrl,
    privacyUrl,
    ...platformCopy({ android, linux, macos, harmony, ios, mobile }),
  } as const;
}
