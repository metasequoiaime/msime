/** 共享扫描器输出的装饰词声明；宿主不再解析原始颜色或清单。 */
export interface DecorationCandidateStyle {
  readonly index: number;
  readonly fontSizeDip: number;
  readonly minFontSizeDip: number;
  readonly maxWidthDip: number;
  readonly resolvedColor: string;
  readonly alignH: string;
  readonly alignV: string;
  readonly offsetXDip: number;
  readonly offsetYDip: number;
  readonly stripBadge: boolean;
}

export interface DecorationWordCandidate {
  readonly text: string;
  readonly badge: string;
  readonly highlighted: boolean;
}

export interface DecorationWordSize {
  readonly width: number;
  readonly height: number;
}

export interface DecorationWordFrame extends DecorationWordSize {
  readonly text: string;
  readonly fontSize: number;
  readonly color: string;
  readonly x: number;
  readonly y: number;
}

export class DecorationCandidatePolicy {
  static valid(style: DecorationCandidateStyle | null | undefined): boolean {
    if (style === null || style === undefined) return false;
    const bounded = (value: number, low: number, high: number): boolean =>
      typeof value === "number" && Number.isFinite(value) && value >= low && value <= high;
    return (
      Number.isInteger(style.index) &&
      bounded(style.index, 0, 100) &&
      bounded(style.fontSizeDip, 0, 200) &&
      style.fontSizeDip > 0 &&
      bounded(style.minFontSizeDip, 0, 200) &&
      style.minFontSizeDip > 0 &&
      bounded(style.maxWidthDip, 0, 2000) &&
      bounded(style.offsetXDip, -2000, 2000) &&
      bounded(style.offsetYDip, -2000, 2000) &&
      typeof style.stripBadge === "boolean" &&
      (style.alignH === "left" || style.alignH === "center" || style.alignH === "right") &&
      (style.alignV === "top" || style.alignV === "center" || style.alignV === "bottom") &&
      typeof style.resolvedColor === "string" &&
      /^#[0-9A-Fa-f]{8}$/.test(style.resolvedColor)
    );
  }

  /** 当前页的独立原文与标注；不猜测原文里括号或后缀的含义。 */
  static word(style: DecorationCandidateStyle, candidates: DecorationWordCandidate[]): string {
    const seat: number =
      style.index === 0
        ? candidates.findIndex(
            (candidate: DecorationWordCandidate): boolean => candidate.highlighted,
          )
        : style.index - 1;
    if (seat < 0 || seat >= candidates.length) return "";
    const candidate: DecorationWordCandidate = candidates[seat];
    return candidate.text + (style.stripBadge ? "" : candidate.badge);
  }

  /** 文字按声明框定位；达到最小字号仍过宽时保留整行，交给父视图溢出绘制。 */
  static frame(
    style: DecorationCandidateStyle | null,
    candidates: DecorationWordCandidate[],
    boxWidth: number,
    boxHeight: number,
    measure: (text: string, fontSize: number) => DecorationWordSize,
  ): DecorationWordFrame | null {
    if (
      !DecorationCandidatePolicy.valid(style) ||
      style === null ||
      !Number.isFinite(boxWidth) ||
      !Number.isFinite(boxHeight) ||
      boxWidth <= 0 ||
      boxHeight <= 0
    )
      return null;
    const text: string = DecorationCandidatePolicy.word(style, candidates);
    if (text.length === 0) return null;
    const natural: DecorationWordSize = measure(text, style.fontSizeDip);
    if (!DecorationCandidatePolicy.measured(natural)) return null;
    const limit: number = style.maxWidthDip > 0 ? style.maxWidthDip : boxWidth;
    const fontSize: number =
      natural.width > limit
        ? Math.max(
            Math.min(style.fontSizeDip, style.minFontSizeDip),
            (style.fontSizeDip * limit) / natural.width,
          )
        : style.fontSizeDip;
    const size: DecorationWordSize =
      fontSize === style.fontSizeDip ? natural : measure(text, fontSize);
    if (!DecorationCandidatePolicy.measured(size)) return null;
    return {
      text: text,
      fontSize: fontSize,
      color: (
        "#" +
        style.resolvedColor.substring(7, 9) +
        style.resolvedColor.substring(1, 7)
      ).toUpperCase(),
      x:
        style.offsetXDip +
        (boxWidth - size.width) *
          (style.alignH === "left" ? 0 : style.alignH === "center" ? 0.5 : 1),
      y:
        style.offsetYDip +
        (boxHeight - size.height) *
          (style.alignV === "top" ? 0 : style.alignV === "center" ? 0.5 : 1),
      width: size.width,
      height: size.height,
    };
  }

  private static measured(size: DecorationWordSize): boolean {
    return (
      Number.isFinite(size.width) &&
      size.width > 0 &&
      Number.isFinite(size.height) &&
      size.height > 0
    );
  }
}
