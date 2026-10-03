/**
 * Touch-keyboard spacing contract, ported from platforms/android/java/app/msime/android/KeyboardGeometry.java.
 * The numbers are the same; only the unit name changes, since HarmonyOS measures in vp where Android
 * measures in dp and both are density-independent.
 *
 * Input algorithms stay in the Engine. This is only the geometry the host draws with.
 */
/** 键的一块触摸矩形相对键左上角的偏移（vp），矩形与键同大。 */
export interface HitOffset {
  readonly x: number;
  readonly y: number;
}

export class KeyboardGeometry {
  static readonly DEFAULT_HEIGHT_ADJUSTMENT_VP: number = 0;
  static readonly MIN_HEIGHT_ADJUSTMENT_VP: number = -12;
  static readonly MAX_HEIGHT_ADJUSTMENT_VP: number = 48;
  static readonly STANDARD_ROW_HEIGHT_VP: number = 48;
  /** Fixed candidate/shortcut row; swapping its contents must not move the key rows. */
  static readonly CANDIDATE_ROW_HEIGHT_VP: number = 48;
  static readonly NINE_KEY_HEIGHT_VP: number = 180;
  static readonly HANDWRITING_BODY_HEIGHT_VP: number = 220;
  static readonly DEFAULT_KEY_SPACING_TENTHS: number = 60;
  static readonly DEFAULT_ROW_SPACING_TENTHS: number = 70;
  static readonly MIN_KEY_SPACING_TENTHS: number = 30;
  static readonly MAX_KEY_SPACING_TENTHS: number = 60;
  static readonly MIN_ROW_SPACING_TENTHS: number = 40;
  static readonly MAX_ROW_SPACING_TENTHS: number = 100;

  static keySpacing(value: number): number {
    return KeyboardGeometry.clamp(
      value,
      KeyboardGeometry.MIN_KEY_SPACING_TENTHS,
      KeyboardGeometry.MAX_KEY_SPACING_TENTHS,
      KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS,
    );
  }

  static rowSpacing(value: number): number {
    return KeyboardGeometry.clamp(
      value,
      KeyboardGeometry.MIN_ROW_SPACING_TENTHS,
      KeyboardGeometry.MAX_ROW_SPACING_TENTHS,
      KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS,
    );
  }

  /**
   * null stands for the unset sentinel the Java side spells as Integer.MIN_VALUE; an absent
   * preference falls back to the default rather than clamping to the minimum.
   */
  static heightAdjustment(value: number | null): number {
    if (value === null) {
      return KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP;
    }
    return Math.max(
      KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP,
      Math.min(value, KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP),
    );
  }

  /** Divide the total adjustment across rows without losing a density-independent pixel. */
  static adjustedRowHeight(
    baseHeight: number,
    adjustment: number | null,
    rowCount: number,
    rowIndex: number,
  ): number {
    if (baseHeight <= 0 || rowCount <= 0 || rowIndex < 0 || rowIndex >= rowCount) {
      throw new Error("Invalid keyboard height geometry");
    }
    const total: number = baseHeight * rowCount + KeyboardGeometry.heightAdjustment(adjustment);
    return Math.floor(total / rowCount) + (rowIndex < total % rowCount ? 1 : 0);
  }

  static display(tenths: number): string {
    return (tenths / 10).toFixed(1);
  }

  static displayHeight(adjustment: number | null): string {
    const value: number = KeyboardGeometry.heightAdjustment(adjustment);
    return (value > 0 ? "+" : "") + value.toString();
  }

  static halfGapPixels(tenths: number, density: number): number {
    if (!Number.isFinite(density) || density <= 0) {
      return 0;
    }
    return Math.max(0, Math.round((tenths * density) / 20));
  }

  /**
   * 键距和行距是容器的 `space`，键帽之间那几 vp 不属于任何键，落在那里的按下没有组件接收，整个手势就丢了。每个键把触摸区向四边的空隙各扩出给定的 vp（通常是那一侧空隙的一半，相邻两键的触摸区正好在空隙中线相接），布局和绘制都不变。
   *
   * ArkUI 的 `responseRegion` 是一组矩形的并集，每块的 `x`、`y` 是相对键左上角的偏移，宽高的百分比按键自身尺寸算，没有「100% 加若干 vp」的写法。所以扩大后的区域用与键同大、按 (-left 或 +right, -top 或 +bottom) 平移的几块矩形拼出：只要两侧扩出之和不超过键的宽高，它们的并集恰好是 `[-left, width + right] x [-top, height + bottom]`。某一侧不扩时对应的平移重合，只留一块。
   */
  static hitOffsets(left: number, top: number, right: number, bottom: number): HitOffset[] {
    const xs: number[] = KeyboardGeometry.shifts(left, right);
    const ys: number[] = KeyboardGeometry.shifts(top, bottom);
    const offsets: HitOffset[] = [];
    for (const y of ys) {
      for (const x of xs) {
        offsets.push({ x: x, y: y });
      }
    }
    return offsets;
  }

  private static shifts(before: number, after: number): number[] {
    const lead: number = -Math.max(0, before);
    const trail: number = Math.max(0, after);
    return lead === trail ? [0] : [lead, trail];
  }

  static bounded(value: number, minimum: number, maximum: number): number {
    return Math.max(minimum, Math.min(value, maximum));
  }

  private static clamp(value: number, minimum: number, maximum: number, fallback: number): number {
    if (value < 0) {
      return fallback;
    }
    return KeyboardGeometry.bounded(value, minimum, maximum);
  }
}
