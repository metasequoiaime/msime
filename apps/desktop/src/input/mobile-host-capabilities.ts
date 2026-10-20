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
    // Windows 和 Linux 的设置应用自己用账号会话做快照：备份走宿主保存（Windows 是下载链接）和页面的文件选择，「应用到本机」在设置应用里请输入会话放开词库后激活（Windows 请 Server，Linux 经 IBus 和 Fcitx5 宿主都认的租约），所以不是 macOS 那条原生路径。
    snapshot:
      isMobileHost(platform) ||
      platform === "macos" ||
      platform === "windows" ||
      platform === "linux",
    snapshotNative: platform === "macos",
    // Windows 的 Server 和 Linux 的 IBus、Fcitx5 宿主都不处理快照队列，入列后由设置应用的面板当场应用、开着时重试；面板据此不说「输入法空闲时应用」。
    ...(platform === "windows" || platform === "linux" ? { snapshotAppliesInPanel: true } : {}),
    ...(platform === "macos" && request
      ? { chooseSnapshotRestore: () => request({ operation: "snapshot_choose_restore" }) }
      : {}),
  };
}
