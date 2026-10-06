/**
 * What differs between input schemes, as the keyboard needs to know it.
 *
 * The Engine's `SchemeType` predicates (crates/engine/src/types.rs) are the one source of truth, but the keyboard decides from the `scheme` number in the Engine's view and cannot call them, so the predicates it needs are copied here. `scripts/test-scheme-traits-parity.py` reads this file and fails when a constant is not the Engine ordinal or a predicate here answers differently from the Engine one it is named after, for any scheme or for a number the Engine does not know.
 *
 * Each predicate is written as the list of schemes it is true for, which is the form the parity script evaluates; keep it that way.
 */
export class SchemeTraits {
  static readonly QUANPIN: number = 0;
  static readonly SHUANGPIN: number = 1;
  static readonly WUBI: number = 2;
  static readonly JAPANESE: number = 3;
  static readonly KOREAN: number = 4;
  static readonly CANTONESE: number = 5;
  static readonly ZHUYIN: number = 6;
  static readonly VIETNAMESE: number = 7;
  static readonly TIBETAN: number = 8;
  static readonly STROKE: number = 9;

  /** The Engine's wire names, indexed by scheme number. */
  static readonly NAMES: string[] = [
    "quanpin",
    "shuangpin",
    "wubi",
    "japanese",
    "korean",
    "cantonese",
    "zhuyin",
    "vietnamese",
    "tibetan",
    "stroke",
  ];

  /** The scheme number for a wire name, or -1 for a name no build knows, which every predicate answers false for. */
  static fromName(name: string): number {
    return SchemeTraits.NAMES.indexOf(name);
  }

  /** A Chinese scheme: what 中文 returns to and what the Chinese statistics count. */
  static isChinese(scheme: number): boolean {
    return [
      SchemeTraits.QUANPIN,
      SchemeTraits.SHUANGPIN,
      SchemeTraits.WUBI,
      SchemeTraits.CANTONESE,
      SchemeTraits.STROKE,
      SchemeTraits.ZHUYIN,
    ].includes(scheme);
  }

  /** The Simplified-to-Traditional switch applies; Cantonese and Zhuyin are Traditional as typed, and Stroke writes each character as msime-stroke.db stores it. */
  static scriptConversionApplies(scheme: number): boolean {
    return [SchemeTraits.QUANPIN, SchemeTraits.SHUANGPIN, SchemeTraits.WUBI].includes(scheme);
  }

  /** 标点走中文标点表；韩语、越南语和藏文写 ASCII 标点。 */
  static usesChinesePunctuation(scheme: number): boolean {
    return [
      SchemeTraits.QUANPIN,
      SchemeTraits.SHUANGPIN,
      SchemeTraits.WUBI,
      SchemeTraits.JAPANESE,
      SchemeTraits.CANTONESE,
      SchemeTraits.STROKE,
      SchemeTraits.ZHUYIN,
    ].includes(scheme);
  }

  /** The host's smart punctuation may run. */
  static hostSmartPunctuation(scheme: number): boolean {
    return [
      SchemeTraits.QUANPIN,
      SchemeTraits.SHUANGPIN,
      SchemeTraits.WUBI,
      SchemeTraits.CANTONESE,
      SchemeTraits.STROKE,
    ].includes(scheme);
  }

  /** Commits are widened when the fullwidth switch is on. */
  static widensFullWidth(scheme: number): boolean {
    return [
      SchemeTraits.QUANPIN,
      SchemeTraits.SHUANGPIN,
      SchemeTraits.WUBI,
      SchemeTraits.JAPANESE,
      SchemeTraits.CANTONESE,
      SchemeTraits.STROKE,
      SchemeTraits.ZHUYIN,
    ].includes(scheme);
  }

  /** Selections are learned into the main dictionary, so a candidate can be pinned, demoted or removed. */
  static learnsIntoMainDictionary(scheme: number): boolean {
    return [SchemeTraits.QUANPIN, SchemeTraits.SHUANGPIN, SchemeTraits.WUBI].includes(scheme);
  }

  /** Leaving the field or the scheme commits the composition instead of discarding it. */
  static commitsOnBlur(scheme: number): boolean {
    return [
      SchemeTraits.KOREAN,
      SchemeTraits.ZHUYIN,
      SchemeTraits.VIETNAMESE,
      SchemeTraits.TIBETAN,
    ].includes(scheme);
  }

  /** Candidates appear only in a list the user opens and can close again: the Korean Hanja list, the Zhuyin list. */
  static hasOpenableCandidateList(scheme: number): boolean {
    return [SchemeTraits.KOREAN, SchemeTraits.ZHUYIN].includes(scheme);
  }

  /** 第一次取消保留组字（关闭候选列表，或把越南语单词、藏文音节退回成原始按键），第二次才丢弃。 */
  static cancelKeepsComposition(scheme: number): boolean {
    return [
      SchemeTraits.KOREAN,
      SchemeTraits.ZHUYIN,
      SchemeTraits.VIETNAMESE,
      SchemeTraits.TIBETAN,
    ].includes(scheme);
  }

  /** The caret stays at the end of the composition. */
  static locksCaret(scheme: number): boolean {
    return [
      SchemeTraits.KOREAN,
      SchemeTraits.ZHUYIN,
      SchemeTraits.VIETNAMESE,
      SchemeTraits.TIBETAN,
    ].includes(scheme);
  }
}
