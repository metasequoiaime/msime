import { SchemeTraits } from "../SchemeTraits";

/**
 * Simplified/Traditional output boundary, ported from
 * platforms/android/java/app/msime/android/ChineseOutputPolicy.java.
 *
 * Only the schemes the switch applies to (`script_conversion_applies`) convert: Japanese, Korean and Vietnamese have nothing to convert, and Cantonese and Zhuyin are Traditional as typed. A converter that fails must not lose the text, so any error falls back to the original.
 */
export type ChineseConverter = (text: string) => string | null;

export class ChineseOutputPolicy {
  /** 来源 18 已由共享运行时逐段转换，保留其中的日文汉字。 */
  static candidateOutput(
    text: string,
    traditional: boolean,
    applies: boolean,
    source: number,
    converter: ChineseConverter,
  ): string {
    return ChineseOutputPolicy.output(text, traditional, applies && source !== 18, converter);
  }
  static applies(dedicatedEnglish: boolean, scheme: number, localMode: string): boolean {
    return (
      !dedicatedEnglish &&
      SchemeTraits.scriptConversionApplies(scheme) &&
      localMode !== "temporary_japanese"
    );
  }

  /** 粤拼和粤语双拼（Engine 里同为 `cantonese`）的词库是繁体：在这个方案下（不在英文模式时），简繁开关切换的是粤拼自己的输出字形 `cantonese_simplified_output`，繁转简在 Engine 里做；全局的 `traditional_chinese_output` 不动。 */
  static switchFollowsCantonese(engineScheme: string, english: boolean): boolean {
    return !english && engineScheme === "cantonese";
  }

  /** 简繁开关显示的字形：粤拼下看粤拼自己的选择（默认繁体），其余方案看全局开关。 */
  static showsTraditional(
    followsCantonese: boolean,
    traditional: boolean,
    cantoneseSimplified: boolean,
  ): boolean {
    return followsCantonese ? !cantoneseSimplified : traditional;
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
