/**
 * Availability of the Japanese post-kana variant key, ported from
 * platforms/android/java/app/msime/android/JapaneseVariantPolicy.java.
 */
export class JapaneseVariantPolicy {
  static enabled(japaneseNineKey: boolean, symbols: boolean, composing: boolean): boolean {
    return japaneseNineKey && !symbols && composing;
  }

  static accessibilityLabel(enabled: boolean): string {
    return enabled ? "小假名、浊音、半浊音" : "小假名、浊音、半浊音；请先输入假名";
  }

  /**
   * Whether the key is the bracket key rather than the variant key.
   *
   * On the digit layer the keys stop producing kana, so the post-modifier has nothing to modify and
   * the twelfth cell would otherwise sit there dead — which is what it did here. The source gives
   * the slot to the brackets, which have no other home on this layout, and the key renames itself
   * 括弧 while it holds them.
   */
  static brackets(japaneseNineKey: boolean, symbols: boolean): boolean {
    return japaneseNineKey && symbols;
  }

  /** Never the pair it draws: 「（）」 read aloud is two characters, not what the key opens. */
  static bracketsLabel(): string {
    return "括弧，可选其他括号";
  }
}
