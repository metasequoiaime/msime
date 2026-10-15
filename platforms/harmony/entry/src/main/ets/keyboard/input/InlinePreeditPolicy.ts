/**
 * 组字写进输入框的预上屏文字（`InputClient.setPreviewText`），编辑框可能支持也可能不支持。
 *
 * 2in1 照共享的 `tsf_preedit_style` 来：Windows 经 TSF 在行内画拼写（`raw` 原始按键、`pinyin` 拼音分词或不画），Linux 宿主读同一个字段。手机与 Apple 键盘、Gboard 一样，组字总是以拼音分词写进输入框，没有开关；候选栏因此不再画读音（见 `hidesStripReading`），候选从行首开始，不随拼写变长左右移动。
 */
/** One of `raw`, `pinyin` or `empty`, as the shared preference spells them. */
export type InlinePreeditStyle = string;

export class InlinePreeditPolicy {
  /** Anything unknown falls back to the shipped default, as the Windows and Linux readers do. */
  static style(value: string | null | undefined): InlinePreeditStyle {
    return value === "pinyin" || value === "empty" ? value : "raw";
  }

  /**
   * 一次 Engine 回报对应的预上屏文字；空串表示不该显示。
   *
   * 短语里已选定的那一段领在拼写前面，与候选窗口的画法相同，已经选好的字在输入框里一直看得见，不会等整个短语上屏才出现；Ctrl+Backspace 清空读音后它单独留着，因为它仍是组字的一部分。
   *
   * 本身就是文字而不是拼写的组字（`writtenInline`：韩语音节、注音转换、越南语单词，见 SchemeCompositionPolicy.drawsPreedit）在任何形态、无论哪种样式都写进输入框：`editing` 只是它背后的按键，不写的话用户在打字的地方什么也看不到。只有不接受预上屏文字的编辑框例外。
   *
   * 手机不看样式，总是写 `preedit`：调用方传入的拼音分词形式，全拼九键是 `nine_key_reading`（`ni'hao`），不是按下的数字。`stripOnly` 的组字不写进输入框，读音留在候选栏上：全拼 14 键的组字是成组的组码，写进输入框对用户没有意义；日语的假名读音保持原来的做法。
   */
  static text(
    style: InlinePreeditStyle,
    desktop: boolean,
    supported: boolean,
    editing: string,
    preedit: string,
    phrasePrefix: string,
    writtenInline: boolean = false,
    stripOnly: boolean = false,
  ): string {
    if (writtenInline) {
      return supported && editing.length > 0 ? preedit : "";
    }
    if (!supported || (editing.length === 0 && phrasePrefix.length === 0)) return "";
    if (!desktop) {
      return stripOnly ? "" : phrasePrefix + (preedit.length > 0 ? preedit : editing);
    }
    if (style === "empty") return "";
    const spelling: string = style === "pinyin" && preedit.length > 0 ? preedit : editing;
    return phrasePrefix + spelling;
  }

  /**
   * 手机候选栏是否不画读音：拼写此刻正以预上屏文字显示在输入框里。
   *
   * 编辑框不接受预上屏文字、或者写入失败时 `preview` 为空，读音照旧画在候选栏上，拼写不会哪里都看不到。本身就是文字的组字（`writtenInline`）照旧也画在候选栏上。2in1 的候选窗口仍按「候选窗口预编辑」来。
   */
  static hidesStripReading(desktop: boolean, preview: string, writtenInline: boolean): boolean {
    return !desktop && preview.length > 0 && !writtenInline;
  }

  /**
   * The document text before the composition, given what the editor reports before the caret.
   *
   * An editor that counts preview text as its content reports the spelling as the text before the caret; a punctuation key deciding on the preceding character must see what the user wrote, not the letters of the composition it is about to replace.
   */
  static beforePreview(forward: string, preview: string): string {
    return preview.length > 0 && forward.endsWith(preview)
      ? forward.substring(0, forward.length - preview.length)
      : forward;
  }
}
