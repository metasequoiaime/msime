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

    /** client-core's default candidate font size (`default_candidate_font_size`). The expanded panel scales its design sizes by the strip's actual font size relative to this value, so the default font at 100% keeps the original 17 sp text and 44 dp cells exactly. */
    public static final int DEFAULT_CANDIDATE_FONT_SIZE = 18;
    /** Text size of an expanded-panel ("more candidates") cell at the default font size and 100%. */
    public static final int EXPANDED_TEXT_SP = 17;
    /** Height of an expanded-panel cell at the default font size and 100%. */
    public static final int EXPANDED_CELL_HEIGHT_DP = 44;
    /** Height of an expanded-panel cell that also draws the pronunciation row, at the default font size and 100%. */
    public static final int EXPANDED_READING_CELL_HEIGHT_DP = 60;

    /**
     * Expanded-panel text size for the strip's actual candidate font size {@code candidateFontSize}, which already includes the candidate bar scale. The panel therefore follows both the candidate font size preference and the candidate bar size, the same way the strip does.
     */
    public static int expandedTextSize(int candidateFontSize) {
        return fontRatio(EXPANDED_TEXT_SP, candidateFontSize);
    }

    /**
     * Expanded-panel cell height for a design height {@code baseDp}: grown by the candidate bar size like the strip row, and by the same ratio as the text when the font size makes the text taller, so a larger font is not clipped vertically. It never drops below {@code baseDp}: the panel takes the keys' place, which do not shrink with the candidate bar, so a smaller bar or font keeps the original touch target, the same floor iOS keeps on its 44 pt cells.
     */
    public static int expandedCellHeight(int baseDp, int candidateFontSize, int percent) {
        return Math.max(baseDp, Math.max(dimension(baseDp, percent), fontRatio(baseDp, candidateFontSize)));
    }

    private static int fontRatio(int value, int candidateFontSize) {
        return Math.max(1, Math.round(value * candidateFontSize / (float) DEFAULT_CANDIDATE_FONT_SIZE));
    }
}
