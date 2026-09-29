/**
 * Keeps the visible letter face separate from the case sent to the Engine, ported from
 * platforms/android/java/app/msime/android/LetterKeyFacePolicy.java.
 *
 * Chinese mode prints uppercase faces while still sending lowercase, which is why the face and the
 * engine input are decided separately.
 */
export class LetterKeyFacePolicy {
  static displaysUppercase(chineseMode: boolean, localMode: boolean, shifted: boolean): boolean {
    return (chineseMode && !localMode) || (!chineseMode && shifted);
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
    return (!chineseMode && shifted ? "大写 " : "字母 ") + lowercase.toUpperCase();
  }
}
