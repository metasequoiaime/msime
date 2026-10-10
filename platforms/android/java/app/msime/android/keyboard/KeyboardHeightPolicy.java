package app.msime.android;

/** Shared keyboard height adjustment and row distribution contract. */
public final class KeyboardHeightPolicy {
    private KeyboardHeightPolicy() { }

    public static final int DEFAULT_HEIGHT_ADJUSTMENT_DP = 0;
    public static final int MIN_HEIGHT_ADJUSTMENT_DP = -12;
    public static final int MAX_HEIGHT_ADJUSTMENT_DP = 48;
    /** 键区的行数：三行键（或一块占三行高的键块）加底行。 */
    public static final int KEYBOARD_ROW_COUNT = 4;

    public static final int MIN_HEIGHT_PERCENT = 75;
    public static final int MAX_HEIGHT_PERCENT = 160;
    public static final int DEFAULT_HEIGHT_PERCENT = 100;
    public static final int HEIGHT_PERCENT_BASE_DP = 184;
    public static final int MIN_DESIGN_HEIGHT_ADJUSTMENT_DP = -46;
    public static final int MAX_DESIGN_HEIGHT_ADJUSTMENT_DP = 110;
    public static final int LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP = 55;
    public static final int WINDOW_HEIGHT_ADJUSTMENT_PERCENT = 14;

    public static int bottomRowAdjustment(int adjustment) {
        return Math.floorDiv(designHeightAdjustment(adjustment), KEYBOARD_ROW_COUNT);
    }

    public static int keyRowsAdjustment(int adjustment) {
        int value = designHeightAdjustment(adjustment);
        return value - bottomRowAdjustment(value);
    }

    public static int heightPercentToAdjustment(int percent) {
        int clamped = BoundsPolicy.bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT);
        int scaled = HEIGHT_PERCENT_BASE_DP * (clamped - DEFAULT_HEIGHT_PERCENT);
        return (scaled + Integer.signum(scaled) * 50) / 100;
    }

    public static int heightAdjustmentToPercent(int adjustment) {
        int clamped = designHeightAdjustment(adjustment);
        int scaled = clamped * 100;
        return DEFAULT_HEIGHT_PERCENT
            + (scaled + Integer.signum(scaled) * HEIGHT_PERCENT_BASE_DP / 2) / HEIGHT_PERCENT_BASE_DP;
    }

    public static int designHeightAdjustment(int value) {
        if (value == Integer.MIN_VALUE) return DEFAULT_HEIGHT_ADJUSTMENT_DP;
        return BoundsPolicy.bounded(value, MIN_DESIGN_HEIGHT_ADJUSTMENT_DP, MAX_DESIGN_HEIGHT_ADJUSTMENT_DP);
    }

    public static int windowHeightAdjustment(int adjustment, int windowHeightDp) {
        int value = designHeightAdjustment(adjustment);
        if (value <= LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP) return value;
        int windowLimit = windowHeightDp <= 0 ? 0
            : (int) ((long) windowHeightDp * WINDOW_HEIGHT_ADJUSTMENT_PERCENT / 100);
        return BoundsPolicy.atMost(value,
            BoundsPolicy.atLeast(windowLimit, LEGACY_MAX_DESIGN_HEIGHT_ADJUSTMENT_DP));
    }

    public static int designKeyHeight(int percent) {
        int clamped = BoundsPolicy.bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT);
        return (KeyboardGeometry.DESIGN_KEY_HEIGHT_DP * clamped + 50) / 100;
    }

    public static String displayPercent(int percent) {
        return BoundsPolicy.bounded(percent, MIN_HEIGHT_PERCENT, MAX_HEIGHT_PERCENT) + "%";
    }

    public static int heightAdjustment(int value) {
        if (value == Integer.MIN_VALUE) return DEFAULT_HEIGHT_ADJUSTMENT_DP;
        return BoundsPolicy.bounded(value, MIN_HEIGHT_ADJUSTMENT_DP, MAX_HEIGHT_ADJUSTMENT_DP);
    }

    public static int adjustedRowHeight(int baseHeight, int adjustment, int rowCount, int rowIndex) {
        if (baseHeight <= 0 || rowCount <= 0 || rowIndex < 0 || rowIndex >= rowCount)
            throw new IllegalArgumentException("Invalid keyboard height geometry");
        int total = baseHeight * rowCount + designHeightAdjustment(adjustment);
        return total / rowCount + (rowIndex < total % rowCount ? 1 : 0);
    }

    public static String displayHeight(int adjustment) {
        int value = heightAdjustment(adjustment);
        return (value > 0 ? "+" : "") + value;
    }
}
