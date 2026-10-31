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

    /** client-core 的默认候选字号（`default_candidate_font_size`）。展开面板按候选栏实际字号相对这个值的比例缩放设计尺寸，因此默认字号 100% 时仍精确保持原来的 17 sp 文字和 44 dp 单元格。 */
    public static final int DEFAULT_CANDIDATE_FONT_SIZE = 18;
    /** 默认字号 100% 时展开面板“更多候选”单元格的文字大小。 */
    public static final int EXPANDED_TEXT_SP = 17;
    /** 默认字号 100% 时展开面板单元格的高度。 */
    public static final int EXPANDED_CELL_HEIGHT_DP = 44;
    /** 默认字号 100% 时同时绘制读音行的展开面板单元格高度。 */
    public static final int EXPANDED_READING_CELL_HEIGHT_DP = 60;

    /**
     * 按候选栏实际字号 {@code candidateFontSize} 计算展开面板文字大小；该字号已经包含候选栏大小，因此面板会和候选栏一样同时跟随字号偏好与候选栏大小。
     */
    public static int expandedTextSize(int candidateFontSize) {
        return fontRatio(EXPANDED_TEXT_SP, candidateFontSize);
    }

    /**
     * 按设计高度 {@code baseDp} 计算展开面板单元格高度：像候选栏一样随候选栏大小增长，字号变大使文字变高时再按文字比例增长，避免大字号被垂直裁切。高度不会低于 {@code baseDp}：面板占据键区位置，键区不会随候选栏缩小，因此小候选栏或小字号仍保留原触摸目标，和 iOS 44 pt 单元格的下限一致。
     */
    public static int expandedCellHeight(int baseDp, int candidateFontSize, int percent) {
        return Math.max(baseDp, Math.max(dimension(baseDp, percent), fontRatio(baseDp, candidateFontSize)));
    }

    private static int fontRatio(int value, int candidateFontSize) {
        return Math.max(1, Math.round(value * candidateFontSize / (float) DEFAULT_CANDIDATE_FONT_SIZE));
    }
}
