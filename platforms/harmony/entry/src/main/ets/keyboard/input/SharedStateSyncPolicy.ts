import { MirrorManifest } from "../SharedStateMirrorPolicy";

/** 键盘获得焦点时共享状态操作的顺序。 */
export type SharedStateSyncAction = "import" | "drain" | "publish";

export class SharedStateSyncPolicy {
  static readonly PREFERENCES_POLL_MS: number = 500;

  static keyboardActions(fullMode: boolean): SharedStateSyncAction[] {
    return fullMode ? ["import", "drain", "publish"] : ["import"];
  }

  /** 偏好指纹不包含修订号，相同内容的保存无需让键盘重建会话。 */
  static preferencesToken(manifest: MirrorManifest | null): string {
    return manifest?.entries.find((entry) => entry.name === "preferences")?.fingerprint ?? "";
  }
}
