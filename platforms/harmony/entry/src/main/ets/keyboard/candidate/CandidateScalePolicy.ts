/**
 * 「候选栏大小」（共享偏好 `candidate_scale_percent`）在触屏键盘上的倍数。范围与 client-core 的校验、Android 的 `CandidateScalePolicy` 相同：50–200 以外（包括缺省和非整数）按 100。
 *
 * 触屏工具条（空闲时的工具栏和输入时的候选栏共用同一行）连同候选字号按它缩放，按键不动（#10137）；2in1 的候选卡片只随字号调整，由调用方传 100。
 */
export class CandidateScalePolicy {
  static readonly DEFAULT_PERCENT: number = 100;
  static readonly MIN_PERCENT: number = 50;
  static readonly MAX_PERCENT: number = 200;

  static percent(value: number | null | undefined): number {
    return typeof value === "number" &&
      Number.isInteger(value) &&
      value >= CandidateScalePolicy.MIN_PERCENT &&
      value <= CandidateScalePolicy.MAX_PERCENT
      ? value
      : CandidateScalePolicy.DEFAULT_PERCENT;
  }

  /** 按倍数缩放一段长度或字号，至少 1。 */
  static scaled(value: number, percent: number): number {
    return Math.max(1, Math.round((value * CandidateScalePolicy.percent(percent)) / 100));
  }

  /** client-core 的默认候选字号（`default_candidate_font_size`）。展开面板按候选栏实际字号相对这个值的比例缩放设计尺寸，因此默认字号 100% 时精确保持设计尺寸。 */
  static readonly DEFAULT_CANDIDATE_FONT_SIZE: number = 18;

  /** 按设计尺寸 `designSize` 和候选栏实际字号计算展开面板“更多候选”文字大小；实际字号已包含候选栏大小，规则与 Android 的 `CandidateScalePolicy.expandedTextSize` 相同（#10400）。 */
  static expandedFontSize(designSize: number, candidateFontSize: number): number {
    return CandidateScalePolicy.fontRatio(designSize, candidateFontSize);
  }

  /** 按设计高度 `designVp` 计算展开面板单元格高度：像候选栏一样随候选栏大小增长，并按文字比例增长，最低保持 `designVp`；面板占据键区位置，小候选栏或小字号仍须保留触摸目标，规则与 Android 的 `CandidateScalePolicy.expandedCellHeight` 相同（#10400）。 */
  static expandedCellHeight(designVp: number, candidateFontSize: number, percent: number): number {
    return Math.max(
      designVp,
      CandidateScalePolicy.scaled(designVp, percent),
      CandidateScalePolicy.fontRatio(designVp, candidateFontSize),
    );
  }

  private static fontRatio(value: number, candidateFontSize: number): number {
    return Math.max(
      1,
      Math.round((value * candidateFontSize) / CandidateScalePolicy.DEFAULT_CANDIDATE_FONT_SIZE),
    );
  }
}
