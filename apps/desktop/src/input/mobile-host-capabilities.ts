import type { CloudDictionaryPanelClient, HostPlatform } from "@msime/ui";

export function isMobileHost(platform: HostPlatform | undefined): boolean {
  return platform === "android" || platform === "ios";
}

export function cloudDictionaryCapabilities(
  platform: HostPlatform | undefined,
  request?: CloudDictionaryPanelClient["request"],
): {
  snapshot: boolean;
  snapshotNative: boolean;
  chooseSnapshotRestore?: CloudDictionaryPanelClient["chooseSnapshotRestore"];
} {
  return {
    snapshot: isMobileHost(platform) || platform === "macos",
    snapshotNative: platform === "macos",
    ...(platform === "macos" && request
      ? { chooseSnapshotRestore: () => request({ operation: "snapshot_choose_restore" }) }
      : {}),
  };
}
