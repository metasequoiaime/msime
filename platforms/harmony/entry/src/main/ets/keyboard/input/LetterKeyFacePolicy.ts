/**
 * Keeps the visible letter face separate from the case sent to the Engine, ported from
 * platforms/android/java/app/msime/android/LetterKeyFacePolicy.java.
 *
 * 中文模式下中文方案（全拼、双拼、五笔、粤拼等）的 26 个字母键面画大写，与搜狗、微信、百度、豆包的拼音 26 键以及 Android、iOS 一致，交给 Engine 的仍是小写拼写。日语罗马字照日文键盘的惯例不画大写；中文的本地输入模式（英文补全、Unicode、快捷短语等）按字面输入，也不画大写。其余情况（英文模式、日语罗马字、本地输入模式）平时画小写，只有下一键真的会输入大写时才画大写；`shifted` 由调用方给出，意思是下一键真的会输入大写，而不只是 Shift 亮着。中文的本地输入模式按字面输入，Shift 不当作大写。越南语、藏文的字母区分大小写，调用方对它们传 `chineseMode = false`，键面按 Shift 画大小写。
 */
export class LetterKeyFacePolicy {
  static displaysUppercase(
    chineseMode: boolean,
    japanese: boolean,
    localMode: boolean,
    shifted: boolean,
  ): boolean {
    return (
      (chineseMode && !japanese && !localMode) ||
      LetterKeyFacePolicy.typesUppercase(chineseMode, localMode, shifted)
    );
  }

  /** 下一键是否真的输入大写；读屏的「大写」只跟它走，不跟中文模式的大写键面走。 */
  static typesUppercase(chineseMode: boolean, localMode: boolean, shifted: boolean): boolean {
    return shifted && !(chineseMode && localMode);
  }

  static face(
    lowercase: string | null,
    chineseMode: boolean,
    japanese: boolean,
    localMode: boolean,
    shifted: boolean,
  ): string {
    if (lowercase === null || lowercase.length === 0) {
      return "";
    }
    return LetterKeyFacePolicy.displaysUppercase(chineseMode, japanese, localMode, shifted)
      ? lowercase.toUpperCase()
      : lowercase;
  }

  static accessibilityLabel(
    lowercase: string | null,
    chineseMode: boolean,
    localMode: boolean,
    shifted: boolean,
  ): string {
    if (lowercase === null || lowercase.length === 0) {
      return "字母";
    }
    return (
      (LetterKeyFacePolicy.typesUppercase(chineseMode, localMode, shifted) ? "大写 " : "字母 ") +
      lowercase.toUpperCase()
    );
  }
}
