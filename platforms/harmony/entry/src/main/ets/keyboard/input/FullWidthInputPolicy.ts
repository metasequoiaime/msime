/**
 * Converts direct printable ASCII input to Unicode fullwidth forms, ported from
 * platforms/android/java/app/msime/android/FullWidthInputPolicy.java.
 *
 * Space maps to the ideographic space rather than to fullwidth space, which is the convention the
 * other hosts follow.
 */
export class FullWidthInputPolicy {
  static output(text: string | null, enabled: boolean): string | null {
    if (!enabled || text === null || text.length === 0) {
      return text;
    }
    let converted: string = "";
    for (const character of text) {
      const original: number = character.codePointAt(0) as number;
      const output: number =
        original === 0x20
          ? 0x3000
          : original >= 0x21 && original <= 0x7e
            ? original + 0xfee0
            : original;
      converted += String.fromCodePoint(output);
    }
    return converted;
  }
}
