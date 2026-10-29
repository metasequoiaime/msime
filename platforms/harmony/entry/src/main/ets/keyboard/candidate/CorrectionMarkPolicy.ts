/**
 * 双拼触屏盲打纠错坐上首选时的显式标记（#10018）。
 *
 * 引擎只在纠错行按首选规则坐上第 0 位时，在 View JSON 的这一行带 `confident_correction: true`；没坐首选的纠错行、普通行都没有这个键。宿主读到它时把这一行的候选字和它的注释、释义都画成标记色，读屏在这一行的朗读末尾加「，已纠正」；原来画在旁边的原始按键注释（corrected_from）照旧，只是也换成标记色。
 *
 * 与 Android（`CandidateAppearance.Palette.correctionFor`）、iOS（候选条与展开网格的 `confidentCorrection`）同一条规则：标记色在没高亮的行上是强调色，在高亮的行上是这一处高亮字色。内置配色里高亮字色本来就是强调色，首选又通常就是高亮的那一行，字色看不出区别，所以注释、释义这些平时画次要色的字也换成标记色，标记靠它们看得见。
 */

/** 一处候选面画字所用的三种配色令牌：普通字色、高亮行字色、强调色。 */
export interface CorrectionSurfaceColors {
  readonly text: string;
  readonly highlightedText: string;
  readonly accent: string;
}

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
   * 标记色：高亮的行用这一处的高亮字色，没高亮的行用强调色。
   *
   * 高亮行不用强调色，是因为皮肤可能把高亮底填成强调色，强调色的字会消失在底色里；高亮字色在那种底上一定看得清，内置配色里它就是强调色。
   */
  static markColor(highlighted: boolean, colors: CorrectionSurfaceColors): string {
    return highlighted ? colors.highlightedText : colors.accent;
  }

  /** 这一行某段字（候选字、注释、释义）的颜色：标记的行换成标记色，其余行用调用方原来的 `base`。 */
  static tint(
    confident: boolean,
    highlighted: boolean,
    base: string,
    colors: CorrectionSurfaceColors,
  ): string {
    return confident ? CorrectionMarkPolicy.markColor(highlighted, colors) : base;
  }

  /** 展开面板一个格子的字色：不标的格子按高亮与否取高亮字色或普通字色，标记的格子取标记色。 */
  static cellTextColor(
    confident: boolean,
    highlighted: boolean,
    colors: CorrectionSurfaceColors,
  ): string {
    return CorrectionMarkPolicy.tint(
      confident,
      highlighted,
      highlighted ? colors.highlightedText : colors.text,
      colors,
    );
  }

  /** 展开面板用哪套令牌：有候选包画布时用候选区配色，与格子的底色同源；没有时格子是按键，用键盘配色。 */
  static expandedSurface(
    canvas: boolean,
    candidate: CorrectionSurfaceColors,
    keyboard: CorrectionSurfaceColors,
  ): CorrectionSurfaceColors {
    return canvas ? candidate : keyboard;
  }
}
