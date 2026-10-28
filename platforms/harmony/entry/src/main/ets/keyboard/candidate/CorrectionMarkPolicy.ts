/**
 * 双拼触屏盲打纠错坐上首选时的显式标记（#10018）。
 *
 * 引擎只在纠错行按首选规则坐上第 0 位时，在 View JSON 的这一行带 `confident_correction: true`；没坐首选的纠错行、普通行都没有这个键。宿主读到它时给这一行画强调色的下划线，读屏在这一行的朗读末尾加「，已纠正」；原来画在旁边的原始按键注释（corrected_from）照旧。
 *
 * 用下划线而不是把字改成强调色：触屏候选栏高亮那一行的字本来就是强调色（`selectedText` 与 `accent` 相同），首选通常就是高亮的那一行，改字色看不出区别。
 */
export class CorrectionMarkPolicy {
  /** 读屏追加在这一行朗读末尾的文字。 */
  static readonly ACCESSIBILITY_SUFFIX: string = "，已纠正";

  /** 只认字面的 `true`：键缺失、为 false 或不是布尔值（旧库、畸形回应）都不标。 */
  static confident(flag: boolean | undefined | null): boolean {
    return flag === true;
  }

  /** 这一行朗读末尾要加的文字，不标时为空串。 */
  static accessibilitySuffix(confident: boolean): string {
    return confident ? CorrectionMarkPolicy.ACCESSIBILITY_SUFFIX : "";
  }

  /**
   * 下划线的颜色，不标时为 null（不画下划线）。
   *
   * 高亮的行用这一行自己的高亮字色：内置配色里它就是强调色；皮肤把高亮底填成强调色时，强调色的下划线会消失在底色里，高亮字色在那种底上一定看得清。没高亮的行用强调色。
   */
  static underlineColor(
    confident: boolean,
    highlighted: boolean,
    accent: string,
    highlightedText: string,
  ): string | null {
    if (!confident) return null;
    return highlighted ? highlightedText : accent;
  }
}
