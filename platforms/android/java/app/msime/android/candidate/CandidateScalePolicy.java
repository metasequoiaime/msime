package app.msime.android;

/** 候选栏整体大小的安全范围和 DIP 缩放。 */
public final class CandidateScalePolicy {
    public static final int DEFAULT_PERCENT = 100;
    public static final int MIN_PERCENT = 50;
    public static final int MAX_PERCENT = 200;
    /** 设置页「候选栏大小」滑块的步长；三端触屏设置用同一范围和步长。 */
    public static final int STEP_PERCENT = 5;

    private CandidateScalePolicy() {}

    public static int percent(int value) {
        return value >= MIN_PERCENT && value <= MAX_PERCENT ? value : DEFAULT_PERCENT;
    }

    public static int dimension(int valueDp, int percent) {
        return Math.max(1, Math.round(valueDp * percent / 100f));
    }
}
