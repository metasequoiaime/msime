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

const COMMAND: LocalInputModeDefinition = mode("COMMAND", "/", "指令", "command");

// 实体键盘在没有组字时以 Shift+V、/ 和 @ 进入这三种，三种都默认关。触屏功能面板只额外给「指令」（`touchModes`）：指令触发词是字母，触屏的字母键面打得出来，进入后九键也换成全键字母面；V 要输入数字和运算符，@ 要从名单里挑，触屏键盘没有相应的键面，所以不给。
const HARDWARE_MODES: LocalInputModeDefinition[] = [
  mode("EXPRESSION", "V", "计算", "expression"),
  COMMAND,
  mode("MENTION", "@", "名单", "mention"),
];

const ALL_MODES: LocalInputModeDefinition[] = MODES.concat(HARDWARE_MODES);

export class LocalInputMode {
  /** The modes the touch tools panel offers as tiles. */
  static readonly MODES: LocalInputModeDefinition[] = MODES;
  /** The modes only a hardware keyboard enters; see HARDWARE_MODES. */
  static readonly HARDWARE_MODES: LocalInputModeDefinition[] = HARDWARE_MODES;

  /**
   * 触屏功能面板「本地输入」列出的模式：MODES，设置里打开了 / 指令（`local_modes.command`，缺省关）时末尾再加「指令」。没打开时 Engine 不进入指令模式，列出来只会是一个点了没用的格子。
   */
  static touchModes(command: boolean): LocalInputModeDefinition[] {
    return command ? MODES.concat([COMMAND]) : MODES;
  }

  /** 点这一项交给 Engine 的键要不要带 Shift：字母触发词按大写进入，`/` 按普通标点键送，与实体键盘没有组字时按 / 一样。 */
  static entersWithShift(definition: LocalInputModeDefinition): boolean {
    return definition.trigger >= "A" && definition.trigger <= "Z";
  }

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
