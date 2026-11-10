import { SchemeTraits } from "../SchemeTraits";

/**
 * 简繁输出边界，移植自
 * platforms/android/java/app/msime/android/ChineseOutputPolicy.java。
 *
 * 全局开关适用于标记了 `script_conversion_applies` 的方案；粤语和注音词库原生为繁体，
 * 因此各自有可选的繁转简输出开关。转换失败时必须保留文本，所以任何错误都回退到原文。
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

  /** 注音词库原生繁体，鸿蒙在同一个简繁按钮下使用独立的输出偏好。 */
  static switchFollowsZhuyin(engineScheme: string, english: boolean): boolean {
    return !english && engineScheme === "zhuyin";
  }

  /** 粤拼下按一次简繁开关后的 `cantonese_simplified_output`：只把它交给 `write`（存储），存储接受了才翻转，拒绝时保持原值；全局的 `traditional_chinese_output` 不写。 */
  static toggledCantoneseSimplified(
    cantoneseSimplified: boolean,
    write: (key: string, value: boolean) => boolean,
  ): boolean {
    return write("cantonese_simplified_output", !cantoneseSimplified)
      ? !cantoneseSimplified
      : cantoneseSimplified;
  }

  /** 注音下按一次简繁开关后的 `zhuyin_simplified_output`。 */
  static toggledZhuyinSimplified(
    zhuyinSimplified: boolean,
    write: (key: string, value: boolean) => boolean,
  ): boolean {
    return write("zhuyin_simplified_output", !zhuyinSimplified)
      ? !zhuyinSimplified
      : zhuyinSimplified;
  }

  /** 简繁开关显示的字形：粤拼下看粤拼自己的选择（默认繁体），其余方案看全局开关。 */
  static showsTraditional(
    followsCantonese: boolean,
    traditional: boolean,
    cantoneseSimplified: boolean,
    followsZhuyin: boolean = false,
    zhuyinSimplified: boolean = false,
  ): boolean {
    if (followsCantonese) return !cantoneseSimplified;
    if (followsZhuyin) return !zhuyinSimplified;
    return traditional;
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
