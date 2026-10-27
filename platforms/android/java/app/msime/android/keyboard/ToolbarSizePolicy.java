package app.msime.android;

/** Android 空闲工具栏的独立尺寸；未显式设置时由调用方继续沿用候选栏高度。 */
public final class ToolbarSizePolicy {
    public static final int MIN_PERCENT = 90;
    public static final int MAX_PERCENT = 150;
    public static final int DEFAULT_PERCENT = 100;
    private static final int BASE_HEIGHT_DP = 56;

    private ToolbarSizePolicy() {}

    public static int percent(int value) {
        return value >= MIN_PERCENT && value <= MAX_PERCENT && value % 10 == 0
            ? value : DEFAULT_PERCENT;
    }

    public static int heightDp(int percent) {
        return Math.round(BASE_HEIGHT_DP * ToolbarSizePolicy.percent(percent) / 100f);
    }

    public static int buttonHeightDp(int percent) {
        return Math.max(44, Math.round(44 * ToolbarSizePolicy.percent(percent) / 100f));
    }

    public static int buttonWidthDp(int base, int percent) {
        return Math.max(base, Math.round(base * ToolbarSizePolicy.percent(percent) / 100f));
    }

    public static int idleSpacerPx(int idleHeight, int composingHeight) {
        return Math.max(0, composingHeight - idleHeight);
    }

    public static int composingSpacerPx(int idleHeight, int composingHeight) {
        return Math.max(0, idleHeight - composingHeight);
    }
}
