/**
 * Touch-keyboard spacing contract, ported from platforms/android/java/app/msime/client/KeyboardGeometry.java.
 * The numbers are the same; only the unit name changes, since HarmonyOS measures in vp where Android
 * measures in dp and both are density-independent.
 *
 * Input algorithms stay in the Engine. This is only the geometry the host draws with.
 */
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
    return KeyboardGeometry.clamp(value, KeyboardGeometry.MIN_KEY_SPACING_TENTHS,
      KeyboardGeometry.MAX_KEY_SPACING_TENTHS, KeyboardGeometry.DEFAULT_KEY_SPACING_TENTHS);
  }

  static rowSpacing(value: number): number {
    return KeyboardGeometry.clamp(value, KeyboardGeometry.MIN_ROW_SPACING_TENTHS,
      KeyboardGeometry.MAX_ROW_SPACING_TENTHS, KeyboardGeometry.DEFAULT_ROW_SPACING_TENTHS);
  }

  /**
   * null stands for the unset sentinel the Java side spells as Integer.MIN_VALUE; an absent
   * preference falls back to the default rather than clamping to the minimum.
   */
  static heightAdjustment(value: number | null): number {
    if (value === null) {
      return KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_VP;
    }
    return Math.max(KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_VP,
      Math.min(value, KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_VP));
  }

  /** Divide the total adjustment across rows without losing a density-independent pixel. */
  static adjustedRowHeight(baseHeight: number, adjustment: number | null, rowCount: number,
                           rowIndex: number): number {
    if (baseHeight <= 0 || rowCount <= 0 || rowIndex < 0 || rowIndex >= rowCount) {
      throw new Error('Invalid keyboard height geometry');
    }
    const total: number = baseHeight * rowCount + KeyboardGeometry.heightAdjustment(adjustment);
    return Math.floor(total / rowCount) + (rowIndex < total % rowCount ? 1 : 0);
  }

  static display(tenths: number): string {
    return (tenths / 10).toFixed(1);
  }

  static displayHeight(adjustment: number | null): string {
    const value: number = KeyboardGeometry.heightAdjustment(adjustment);
    return (value > 0 ? '+' : '') + value.toString();
  }

  static halfGapPixels(tenths: number, density: number): number {
    if (!Number.isFinite(density) || density <= 0) {
      return 0;
    }
    return Math.max(0, Math.round(tenths * density / 20));
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
