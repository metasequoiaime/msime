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
  snapshotAppliesInPanel?: boolean;
  chooseSnapshotRestore?: CloudDictionaryPanelClient["chooseSnapshotRestore"];
} {
  return {
    // Windows 的设置应用自己用账号会话做快照：备份走下载链接和页面的文件选择，「应用到本机」在设置应用里请 Server 放开会话后激活，所以不是 macOS 那条原生路径。
    snapshot: isMobileHost(platform) || platform === "macos" || platform === "windows",
    snapshotNative: platform === "macos",
    // Windows 的 Server 不处理快照队列，入列后由设置应用的面板当场应用、开着时重试；面板据此不说「输入法空闲时应用」。
    ...(platform === "windows" ? { snapshotAppliesInPanel: true } : {}),
    ...(platform === "macos" && request
      ? { chooseSnapshotRestore: () => request({ operation: "snapshot_choose_restore" }) }
      : {}),
  };
}
