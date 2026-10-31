package app.msime.android;

public final class CandidateScalePolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    public static void main(String[] args) {
        check(CandidateScalePolicy.percent(50) == 50);
        check(CandidateScalePolicy.percent(200) == 200);
        check(CandidateScalePolicy.percent(49) == 100);
        check(CandidateScalePolicy.percent(201) == 100);
        check(CandidateScalePolicy.dimension(42, 100) == 42);
        check(CandidateScalePolicy.dimension(42, 150) == 63);
        check(CandidateScalePolicy.dimension(42, 50) == 21);
        // 设置页滑块与 client-core 校验的 50–200 一致，步长 5 能停在默认的 100%。
        check(CandidateScalePolicy.MIN_PERCENT == 50 && CandidateScalePolicy.MAX_PERCENT == 200);
        check((CandidateScalePolicy.DEFAULT_PERCENT - CandidateScalePolicy.MIN_PERCENT) % CandidateScalePolicy.STEP_PERCENT == 0);
        expandedPanelFollowsStripFont();
    }

    /** The expanded panel follows the strip's actual font size (font size preference times candidate bar size); #10400. */
    private static void expandedPanelFollowsStripFont() {
        int base = CandidateScalePolicy.EXPANDED_CELL_HEIGHT_DP;
        int reading = CandidateScalePolicy.EXPANDED_READING_CELL_HEIGHT_DP;
        // The default font (18, from client-core) at 100% keeps the original 17 sp text and 44 / 60 dp cells.
        int defaultFont = CandidateScalePolicy.dimension(CandidateScalePolicy.DEFAULT_CANDIDATE_FONT_SIZE, 100);
        check(CandidateScalePolicy.expandedTextSize(defaultFont) == 17);
        check(CandidateScalePolicy.expandedCellHeight(base, defaultFont, 100) == 44);
        check(CandidateScalePolicy.expandedCellHeight(reading, defaultFont, 100) == 60);
        // The candidate bar size scales text and cells together, matching dimension(17 / 44, percent).
        for (int percent : new int[] {50, 75, 150, 200}) {
            int font = CandidateScalePolicy.dimension(CandidateScalePolicy.DEFAULT_CANDIDATE_FONT_SIZE, percent);
            check(Math.abs(CandidateScalePolicy.expandedTextSize(font)
                - CandidateScalePolicy.dimension(CandidateScalePolicy.EXPANDED_TEXT_SP, percent)) <= 1);
            check(CandidateScalePolicy.expandedCellHeight(base, font, percent)
                >= CandidateScalePolicy.dimension(base, percent));
        }
        check(CandidateScalePolicy.expandedTextSize(CandidateScalePolicy.dimension(18, 200)) == 34);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(18, 200), 200) == 88);
        // A smaller candidate bar shrinks the text but keeps the 44 / 60 dp cells, so the touch target stays.
        check(CandidateScalePolicy.expandedTextSize(CandidateScalePolicy.dimension(18, 50)) == 9);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(18, 50), 50) == 44);
        check(CandidateScalePolicy.expandedCellHeight(reading, CandidateScalePolicy.dimension(18, 75), 75) == 60);
        // A larger font preference alone enlarges the text and raises the cell by the same ratio.
        check(CandidateScalePolicy.expandedTextSize(24) == 23);
        check(CandidateScalePolicy.expandedCellHeight(base, 24, 100) == 59);
        check(CandidateScalePolicy.expandedTextSize(32) == 30);
        check(CandidateScalePolicy.expandedCellHeight(reading, 32, 100) == 107);
        // A smaller font shrinks the text but keeps the cell at the design height, and a larger bar still raises it.
        check(CandidateScalePolicy.expandedTextSize(12) == 11);
        check(CandidateScalePolicy.expandedCellHeight(base, 12, 100) == 44);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(12, 200), 200) == 88);
        // Both preferences at their maximum still give a positive, larger cell than the text.
        int largest = CandidateScalePolicy.dimension(32, CandidateScalePolicy.MAX_PERCENT);
        check(CandidateScalePolicy.expandedTextSize(largest) == 60);
        check(CandidateScalePolicy.expandedCellHeight(base, largest, CandidateScalePolicy.MAX_PERCENT) == 156);
        check(CandidateScalePolicy.expandedTextSize(1) >= 1);
    }
}
