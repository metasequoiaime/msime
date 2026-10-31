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

  /** client-core's default candidate font size (`default_candidate_font_size`). The expanded panel scales its design sizes by the strip's actual font size relative to this value, so the default font at 100% keeps the design sizes exactly. */
  static readonly DEFAULT_CANDIDATE_FONT_SIZE: number = 18;

  /** Expanded-panel ("more candidates") text size for the design size `designSize` and the strip's actual candidate font size, which already includes the candidate bar scale; the same rule as Android's `CandidateScalePolicy.expandedTextSize` (#10400). */
  static expandedFontSize(designSize: number, candidateFontSize: number): number {
    return CandidateScalePolicy.fontRatio(designSize, candidateFontSize);
  }

  /** Expanded-panel cell height for the design height `designVp`: grown by the candidate bar size like the strip and by the text's font ratio, never below `designVp`, because the panel takes the keys' place and a smaller bar or font must keep the touch target; the same rule as Android's `CandidateScalePolicy.expandedCellHeight` (#10400). */
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
