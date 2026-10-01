import { SchemeTraits } from "../SchemeTraits";

/**
 * Simplified/Traditional output boundary, ported from
 * platforms/android/java/app/msime/android/ChineseOutputPolicy.java.
 *
 * Only the schemes the switch applies to (`script_conversion_applies`) convert: Japanese, Korean and Vietnamese have nothing to convert, and Cantonese and Zhuyin are Traditional as typed. A converter that fails must not lose the text, so any error falls back to the original.
 */
export type ChineseConverter = (text: string) => string | null;

export class ChineseOutputPolicy {
  static applies(dedicatedEnglish: boolean, scheme: number, localMode: string): boolean {
    return (
      !dedicatedEnglish &&
      SchemeTraits.scriptConversionApplies(scheme) &&
      localMode !== "temporary_japanese"
    );
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
