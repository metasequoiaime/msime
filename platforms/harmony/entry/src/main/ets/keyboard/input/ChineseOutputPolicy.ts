/**
 * Simplified/Traditional output boundary, ported from
 * platforms/android/java/app/msime/android/ChineseOutputPolicy.java.
 *
 * Scheme 3 is Japanese, which has nothing to convert. A converter that fails must not lose the text,
 * so any error falls back to the original.
 */
export type ChineseConverter = (text: string) => string | null;

export class ChineseOutputPolicy {
  static applies(dedicatedEnglish: boolean, scheme: number, localMode: string): boolean {
    return !dedicatedEnglish && scheme !== 3 && localMode !== "temporary_japanese";
  }

  static output(
    text: string,
    traditional: boolean,
    applies: boolean,
    converter: ChineseConverter,
  ): string {
    if (!traditional || !applies || text.length === 0) {
      return text;
    }
    try {
      const converted: string | null = converter(text);
      return converted === null ? text : converted;
    } catch (error) {
      return text;
    }
  }
}
