/**
 * Local input mode shortcuts the shared Engine exposes, ported from
 * platforms/android/java/app/msime/client/LocalInputMode.java.
 *
 * The trigger is the uppercase letter that enters the mode; the preference key is what the shared
 * settings store spells it as.
 */
export interface LocalInputModeDefinition {
  readonly id: string;
  readonly trigger: string;
  readonly title: string;
  readonly preferenceKey: string;
}

function mode(
  id: string,
  trigger: string,
  title: string,
  preferenceKey: string,
): LocalInputModeDefinition {
  return { id: id, trigger: trigger, title: title, preferenceKey: preferenceKey };
}

const MODES: LocalInputModeDefinition[] = [
  mode("UNICODE", "U", "Unicode 码点", "unicode"),
  mode("DATE_TIME", "T", "日期时间", "date_time"),
  mode("SUPER_JIANPIN", "J", "超级简拼", "super_jianpin"),
  mode("QUICK_PHRASE", "K", "快捷短语", "quick_phrase"),
  mode("TEMPORARY_ENGLISH", "Y", "英文补全", "temporary_english"),
  mode("EMOJI", "E", "表情", "emoji"),
  mode("KAOMOJI", "M", "颜文字", "kaomoji"),
  mode("TEMPORARY_JAPANESE", "R", "临时日语", "temporary_japanese"),
];

export class LocalInputMode {
  static readonly MODES: LocalInputModeDefinition[] = MODES;

  static fromTrigger(trigger: string): LocalInputModeDefinition | null {
    for (const candidate of MODES) {
      if (candidate.trigger === trigger) {
        return candidate;
      }
    }
    return null;
  }

  static fromPreferenceKey(key: string): LocalInputModeDefinition | null {
    for (const candidate of MODES) {
      if (candidate.preferenceKey === key) {
        return candidate;
      }
    }
    return null;
  }
}
