import { NineKeyFlickPolicy } from "../input/NineKeyFlickPolicy";

interface PreferenceSnapshot {
  format_version: number;
  revision: number;
  preferences: Record<string, Object>;
}

interface PreferenceReply {
  ok: boolean;
  value?: PreferenceSnapshot;
}

export interface FlickThresholdSettingsState {
  threshold: number;
  ready: boolean;
  error: string;
}

/** 原生滑块读写共享偏好；每次保存只修改最新快照中的一个字段。 */
export class FlickThresholdSettings {
  constructor(
    private load: () => string,
    private save: (revision: number, document: string) => string,
  ) {}

  private snapshot(reply: string): PreferenceSnapshot {
    const parsed: PreferenceReply = JSON.parse(reply) as PreferenceReply;
    const value: PreferenceSnapshot | undefined = parsed.value;
    if (
      parsed.ok !== true || value === undefined || value === null ||
      value.format_version !== 1 || !Number.isSafeInteger(value.revision) || value.revision < 0 ||
      typeof value.preferences !== "object" || value.preferences === null ||
      Array.isArray(value.preferences)
    ) {
      throw new Error("preferences_unavailable");
    }
    return value;
  }

  private state(snapshot: PreferenceSnapshot): FlickThresholdSettingsState {
    return {
      threshold: NineKeyFlickPolicy.threshold(snapshot.preferences["touch_nine_key_flick_threshold"]),
      ready: true,
      error: "",
    };
  }

  read(): FlickThresholdSettingsState {
    try {
      return this.state(this.snapshot(this.load()));
    } catch {
      return { threshold: NineKeyFlickPolicy.DEFAULT_THRESHOLD_VP,
        ready: false, error: "读取设置失败，请重试" };
    }
  }

  write(threshold: number): FlickThresholdSettingsState {
    if (!Number.isFinite(threshold) || NineKeyFlickPolicy.threshold(threshold) !== threshold) {
      const current: FlickThresholdSettingsState = this.read();
      current.error = "滑动距离须为 14–48 之间的偶数";
      return current;
    }
    try {
      const snapshot: PreferenceSnapshot = this.snapshot(this.load());
      snapshot.preferences["touch_nine_key_flick_threshold"] = threshold;
      return this.state(this.snapshot(this.save(snapshot.revision, JSON.stringify(snapshot))));
    } catch {
      const current: FlickThresholdSettingsState = this.read();
      current.error = "保存失败或设置已变化，请重试";
      return current;
    }
  }
}
