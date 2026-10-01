/**
 * What a screen reader says for each key, which is not what the key face says.
 *
 * A key face is as short as it can be and often not a word at all: `⇧`, `123`, `，`, `中`. Read
 * aloud those are nothing, a number, a comma and a character with no context. The source names
 * every one of them, and the names are deliberately about what the key *does* rather than what it
 * shows — `中` reads as 切换中英文, not as the character 中.
 *
 * Several of these labels were already ported into this host: `LetterKeyFacePolicy`,
 * `EnglishLetterCaseState`, `JapaneseVariantPolicy` and `CandidateGlossPolicy` each compute one.
 * They had no callers outside their own tests, because nothing in the view ever attached them —
 * the labels existed and the keyboard said nothing. This file adds the names those four did not
 * cover, and the view now attaches all of them.
 */

/** Ported from the source's key construction, where each of these is an `accessibilityLabel`. */
export class KeyAccessibilityPolicy {
  /** A symbol read as a symbol: the glyph alone is often not pronounceable. */
  static symbol(face: string): string {
    return face.length === 0 ? "符号" : `符号 ${face}`;
  }

  static delete(): string {
    return "删除";
  }

  /**
   * The space bar, which says what it will do rather than what it is.
   *
   * Mid-composition it selects the highlighted candidate, and the source reads it that way — a
   * user who cannot see the candidate row has no other way to know the key has changed meaning.
   */
  static space(composing: boolean): string {
    return composing ? "选定" : "空格";
  }

  /** The return key, read as whatever it is about to do. The face already carries that word. */
  static returnKey(face: string): string {
    return face.length === 0 ? "换行" : face;
  }

  /** Never the mode it is in: a label that read 中 would be the character, not the action. */
  static language(): string {
    return "切换中英文";
  }

  /** The language switch where it draws the mode rather than a key face, as the strip pill and the tool tile do: the action first, then the mode it is in, because "已开启" would not say which of the two languages is on. */
  static languageState(english: boolean): string {
    return `${KeyAccessibilityPolicy.language()}，当前${english ? "英文" : "中文"}`;
  }

  /** The layout toggle, in whichever direction it is pointing. */
  static layoutToggle(showingSymbols: boolean): string {
    return showingSymbols ? "切换到所选输入方案" : "切换到数字和符号";
  }

  /** The comma key, whose long press opens the punctuation list. */
  static punctuation(): string {
    return "常用标点";
  }

  static symbolPanel(): string {
    return "符号面板";
  }

  static scheme(): string {
    return "选择输入方案";
  }

  /** The tool that opens the global theme picker. */
  static theme(): string {
    return "选择主题";
  }

  static tools(): string {
    return "更多快捷设置";
  }

  // The rest of the shortcut bar. Each says what the button does rather than what it looks like,
  // as the keys above do; the reply entry is conditional on its scheme but follows the same rule.
  static emoji(): string {
    return "表情与符号";
  }

  static voice(): string {
    return "语音输入";
  }

  static reply(): string {
    return "生成高情商回复";
  }

  static geometry(): string {
    return "键盘大小与间距";
  }

  static dismiss(): string {
    return "收起键盘";
  }

  /** The strip's punctuation pill and the panel's tile, which switch between Chinese and ASCII punctuation. */
  static punctuationWidth(): string {
    return "切换中英文标点";
  }

  /** The punctuation pill, which draws the mode in force, read with it. */
  static punctuationState(chinese: boolean): string {
    return `${KeyAccessibilityPolicy.punctuationWidth()}，当前${chinese ? "中文标点" : "英文标点"}`;
  }

  /** The 漢 button over a composing Korean syllable, read as what the next tap does: list the syllable's Hanja, or close the open list. */
  static hanja(listOpen: boolean): string {
    return listOpen ? "关闭汉字列表" : "转换为汉字";
  }

  /** The 選 button over a Zhuyin conversion, read as what the next tap does: open the candidate list, or close it. */
  static zhuyinList(listOpen: boolean): string {
    return listOpen ? "关闭候选列表" : "选字";
  }

  /** The candidate translation switch, named as the panel's 译 tile titles it; the strip's 译 pill reads the same name. */
  static translations(): string {
    return "显示译文";
  }

  /** What a screen reader says for a tool tile: its name, and for a switch whether it is on. */
  static tile(title: string, label: string | undefined, on: boolean, toggles: boolean): string {
    const name: string = label ?? title;
    return toggles ? `${name}，${on ? "已开启" : "已关闭"}` : name;
  }

  /** The strip's gear and the panel's tile, which open the settings application. */
  static settings(): string {
    return "打开设置";
  }

  /**
   * One candidate, numbered as it is shown.
   *
   * The number is part of the label because it is how the user selects it on a hardware keyboard
   * and how the row is described in the source. `hint` is the spelling still to be typed, which
   * turns "this word" into "this word, if you keep going" — the difference between a candidate
   * that commits now and one that does not.
   */
  static candidate(number: number, display: string, hint: string, suffix: string): string {
    const base =
      hint.length === 0
        ? `候选词 ${number}：${display}`
        : `候选词 ${number}：${display}，还需输入 ${hint}`;
    return base + suffix;
  }

  /** English completions are candidates too, even though they do not come from the Engine row. */
  static englishSuggestion(number: number, word: string): string {
    return `英文补全 ${number}：${word}`;
  }

  /** The nine-key spelling strip is selectable, not merely a visual preview. */
  static spelling(value: string): string {
    return `选择拼音 ${value}`;
  }
}
