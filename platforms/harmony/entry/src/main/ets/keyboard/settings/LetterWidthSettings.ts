import {
  FourteenKeyLetterWidthMode,
  fourteenKeyLetterWidthMode,
} from "../input/FourteenKeyLetterWidths";

interface PreferenceSnapshot {
  revision: number;
  preferences: Record<string, Object>;
}

interface PreferenceReply {
  ok: boolean;
  value?: PreferenceSnapshot;
}

export interface LetterWidthSettingsState {
  mode: FourteenKeyLetterWidthMode;
  ready: boolean;
  error: string;
}

/** 原生控件每次保存都从共享快照改一个字段，用修订号保护并发修改。 */
export class LetterWidthSettings {
  constructor(
    private load: () => string,
    private save: (revision: number, document: string) => string,
  ) {}

  private snapshot(reply: string): PreferenceSnapshot {
    const parsed: PreferenceReply = JSON.parse(reply) as PreferenceReply;
    const value: PreferenceSnapshot | undefined = parsed.value;
    if (
      parsed.ok !== true ||
      value === undefined ||
      value === null ||
      !Number.isSafeInteger(value.revision) ||
      value.revision < 0 ||
      typeof value.preferences !== "object" ||
      value.preferences === null ||
      Array.isArray(value.preferences)
    ) {
      throw new Error("preferences_unavailable");
    }
    return value;
  }

  read(): LetterWidthSettingsState {
    try {
      const snapshot: PreferenceSnapshot = this.snapshot(this.load());
      return {
        mode: fourteenKeyLetterWidthMode(snapshot.preferences["touch_fourteen_key_letter_widths"]),
        ready: true,
        error: "",
      };
    } catch {
      return { mode: "equal", ready: false, error: "读取设置失败，请重试" };
    }
  }

  write(mode: FourteenKeyLetterWidthMode): LetterWidthSettingsState {
    try {
      const snapshot: PreferenceSnapshot = this.snapshot(this.load());
      snapshot.preferences["touch_fourteen_key_letter_widths"] = mode;
      const saved: PreferenceSnapshot = this.snapshot(
        this.save(snapshot.revision, JSON.stringify(snapshot)),
      );
      return {
        mode: fourteenKeyLetterWidthMode(saved.preferences["touch_fourteen_key_letter_widths"]),
        ready: true,
        error: "",
      };
    } catch {
      const current: LetterWidthSettingsState = this.read();
      current.error = "保存失败或设置已变化，请重试";
      return current;
    }
  }
}
