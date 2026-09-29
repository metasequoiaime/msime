import type { Preferences, Snapshot } from "../index";

export interface SettingsDirtyOptions {
  draft?: Preferences;
  snapshot?: Snapshot;
  macosWubiAutoCommitUnique?: boolean;
  savedMacosWubiAutoCommitUnique?: boolean;
}

/** Reports whether shared settings or the macOS native Wubi preference changed. */
export function settingsDirty({
  draft,
  snapshot,
  macosWubiAutoCommitUnique,
  savedMacosWubiAutoCommitUnique,
}: SettingsDirtyOptions): boolean {
  return (
    (!!draft && !!snapshot && JSON.stringify(draft) !== JSON.stringify(snapshot.preferences)) ||
    (macosWubiAutoCommitUnique !== undefined &&
      macosWubiAutoCommitUnique !== savedMacosWubiAutoCommitUnique)
  );
}
