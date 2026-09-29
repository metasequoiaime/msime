/**
 * Host-side gate for sending an uppercase letter to the Engine as composition helpcode, ported from
 * platforms/android/java/app/msime/client/ChineseHelpcodePolicy.java.
 *
 * Scheme 0 and 1 are quanpin and shuangpin; helpcode means nothing in the others.
 */
export class ChineseHelpcodePolicy {
  static eligible(
    dedicatedEnglish: boolean,
    editingText: string | null,
    scheme: number,
    localMode: string,
  ): boolean {
    return (
      !dedicatedEnglish &&
      editingText !== null &&
      editingText.length > 0 &&
      localMode === "none" &&
      (scheme === 0 || scheme === 1)
    );
  }

  static entersHelpcode(
    dedicatedEnglish: boolean,
    shifted: boolean,
    editingText: string | null,
    scheme: number,
    localMode: string,
  ): boolean {
    return (
      shifted && ChineseHelpcodePolicy.eligible(dedicatedEnglish, editingText, scheme, localMode)
    );
  }
}
