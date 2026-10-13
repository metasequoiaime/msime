/**
 * Local input mode shortcuts the shared Engine exposes, ported from
 * platforms/android/java/app/msime/android/LocalInputMode.java.
 *
 * The trigger is the uppercase letter, or for `/` and `@` the mark, that enters the mode; the preference key is what the shared settings store spells it as, which is also the Engine's `local_mode` name.
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
  // 拆字反查：输入字的各个部件的拼音找不会读的字（huohuohuo 得 焱），追加在 Apple 那八项之后。
  mode("CHAIZI", "C", "拆字", "chaizi"),
];

// Entered from a hardware keyboard only, with Shift+V on an empty composition or `/` and `@` with nothing composed, so they are named but not offered as tiles: the touch tools panel lists MODES, and these three spell with digits, operators and a list the touch keyboard has no face for. All three are off until the user turns them on.
const HARDWARE_MODES: LocalInputModeDefinition[] = [
  mode("EXPRESSION", "V", "计算", "expression"),
  mode("COMMAND", "/", "指令", "command"),
  mode("MENTION", "@", "名单", "mention"),
];

const ALL_MODES: LocalInputModeDefinition[] = MODES.concat(HARDWARE_MODES);

export class LocalInputMode {
  /** The modes the touch tools panel offers as tiles. */
  static readonly MODES: LocalInputModeDefinition[] = MODES;
  /** The modes only a hardware keyboard enters; see HARDWARE_MODES. */
  static readonly HARDWARE_MODES: LocalInputModeDefinition[] = HARDWARE_MODES;

  static fromTrigger(trigger: string): LocalInputModeDefinition | null {
    for (const candidate of ALL_MODES) {
      if (candidate.trigger === trigger) {
        return candidate;
      }
    }
    return null;
  }

  static fromPreferenceKey(key: string): LocalInputModeDefinition | null {
    for (const candidate of ALL_MODES) {
      if (candidate.preferenceKey === key) {
        return candidate;
      }
    }
    return null;
  }
}
