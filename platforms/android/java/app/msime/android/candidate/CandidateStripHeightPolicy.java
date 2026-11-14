package app.msime.android;

/** 候选条按实际需要的释义行数预留高度；无释义时不再空出一行。 */
public final class CandidateStripHeightPolicy {
    public static final int PLAIN_LINE_DP = 32;

    private CandidateStripHeightPolicy() { }

    public static int designHeightDp(int glossRows) {
        return glossRows <= 0 ? PLAIN_LINE_DP
            : ImeToolbar.CANDIDATE_LINE_DP + (glossRows - 1) * ImeToolbar.EXTRA_GLOSS_ROW_DP;
    }

    /** `contentHeightDp` 是含释义和回退字体的实际候选标签排版高度。 */
    public static int heightDp(int glossRows, int percent, float contentHeightDp) {
        return heightDp(glossRows, percent, contentHeightDp, 0);
    }

    public static int heightDp(int glossRows, int percent, float contentHeightDp, int verticalPaddingDp) {
        int scale = CandidateScalePolicy.percent(percent);
        int design = CandidateScalePolicy.dimension(designHeightDp(glossRows), scale);
        if (!Float.isFinite(contentHeightDp) || contentHeightDp <= 0) return design;
        int padding = Math.max(verticalPaddingDp, CandidateScalePolicy.dimension(4, scale));
        return Math.max(design, (int) Math.ceil(contentHeightDp) + padding);
    }
}
