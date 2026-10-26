package app.msime.android;

/** 候选栏整体大小的安全范围和 DIP 缩放。 */
public final class CandidateScalePolicy {
    private static final int DEFAULT_PERCENT = 100;
    private static final int MIN_PERCENT = 50;
    private static final int MAX_PERCENT = 200;

    private CandidateScalePolicy() {}

    public static int percent(int value) {
        return value >= MIN_PERCENT && value <= MAX_PERCENT ? value : DEFAULT_PERCENT;
    }

    public static int dimension(int valueDp, int percent) {
        return Math.max(1, Math.round(valueDp * percent / 100f));
    }
}
