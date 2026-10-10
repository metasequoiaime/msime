import { deepEqual } from "../core/deep-equal";
import type { Preferences, Snapshot } from "../index";

export interface SettingsDirtyOptions {
  draft?: Preferences;
  snapshot?: Snapshot;
  macosShuangpinKeymap?: boolean;
  savedMacosShuangpinKeymap?: boolean;
}

/** 判断共享设置或某项 macOS 原生偏好是否发生了变化。 */
export function settingsDirty({
  draft,
  snapshot,
  macosShuangpinKeymap,
  savedMacosShuangpinKeymap,
}: SettingsDirtyOptions): boolean {
  return (
    (!!draft && !!snapshot && !deepEqual(draft, snapshot.preferences)) ||
    (macosShuangpinKeymap !== undefined && macosShuangpinKeymap !== savedMacosShuangpinKeymap)
  );
}
