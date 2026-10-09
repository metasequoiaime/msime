/**
 * Keeps the visible letter face separate from the case sent to the Engine, ported from
 * platforms/android/java/app/msime/android/LetterKeyFacePolicy.java.
 *
 * 设计稿在所有模式下都画小写键面，与 Android 和 iOS 一致；`shifted` 由调用方给出，意思是下一键真的会输入大写，而不只是 Shift 亮着。中文的本地输入模式例外，那里敲进去的是字面的小写字母，键面固定画小写。
 */
export class LetterKeyFacePolicy {
  static displaysUppercase(chineseMode: boolean, localMode: boolean, shifted: boolean): boolean {
    return shifted && !(chineseMode && localMode);
  }

  static face(
    lowercase: string | null,
    chineseMode: boolean,
    localMode: boolean,
    shifted: boolean,
  ): string {
    if (lowercase === null || lowercase.length === 0) {
      return "";
    }
    return LetterKeyFacePolicy.displaysUppercase(chineseMode, localMode, shifted)
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
      (LetterKeyFacePolicy.displaysUppercase(chineseMode, localMode, shifted) ? "大写 " : "字母 ") +
      lowercase.toUpperCase()
    );
  }
}
