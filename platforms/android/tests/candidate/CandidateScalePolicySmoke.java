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

    /** 展开面板跟随候选栏实际字号（字号偏好乘候选栏大小）；#10400。 */
    private static void expandedPanelFollowsStripFont() {
        int base = CandidateScalePolicy.EXPANDED_CELL_HEIGHT_DP;
        int reading = CandidateScalePolicy.EXPANDED_READING_CELL_HEIGHT_DP;
        // client-core 的默认字号 18 在 100% 时保持原来的 17 sp 文字和 44 / 60 dp 单元格。
        int defaultFont = CandidateScalePolicy.dimension(CandidateScalePolicy.DEFAULT_CANDIDATE_FONT_SIZE, 100);
        check(CandidateScalePolicy.expandedTextSize(defaultFont) == 17);
        check(CandidateScalePolicy.expandedCellHeight(base, defaultFont, 100) == 44);
        check(CandidateScalePolicy.expandedCellHeight(reading, defaultFont, 100) == 60);
        // 候选栏大小同时缩放文字和单元格，与 dimension(17 / 44, percent) 一致。
        for (int percent : new int[] {50, 75, 150, 200}) {
            int font = CandidateScalePolicy.dimension(CandidateScalePolicy.DEFAULT_CANDIDATE_FONT_SIZE, percent);
            check(Math.abs(CandidateScalePolicy.expandedTextSize(font)
                - CandidateScalePolicy.dimension(CandidateScalePolicy.EXPANDED_TEXT_SP, percent)) <= 1);
            check(CandidateScalePolicy.expandedCellHeight(base, font, percent)
                >= CandidateScalePolicy.dimension(base, percent));
        }
        check(CandidateScalePolicy.expandedTextSize(CandidateScalePolicy.dimension(18, 200)) == 34);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(18, 200), 200) == 88);
        // 较小的候选栏缩小文字但保持 44 / 60 dp 单元格，触摸目标不变。
        check(CandidateScalePolicy.expandedTextSize(CandidateScalePolicy.dimension(18, 50)) == 9);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(18, 50), 50) == 44);
        check(CandidateScalePolicy.expandedCellHeight(reading, CandidateScalePolicy.dimension(18, 75), 75) == 60);
        // 仅增大字号偏好时，文字和单元格按相同比例增大。
        check(CandidateScalePolicy.expandedTextSize(24) == 23);
        check(CandidateScalePolicy.expandedCellHeight(base, 24, 100) == 59);
        check(CandidateScalePolicy.expandedTextSize(32) == 30);
        check(CandidateScalePolicy.expandedCellHeight(reading, 32, 100) == 107);
        // 较小字号缩小文字但保持设计高度，候选栏变大时单元格仍会增高。
        check(CandidateScalePolicy.expandedTextSize(12) == 11);
        check(CandidateScalePolicy.expandedCellHeight(base, 12, 100) == 44);
        check(CandidateScalePolicy.expandedCellHeight(base, CandidateScalePolicy.dimension(12, 200), 200) == 88);
        // 两项偏好都取最大值时，单元格仍保持合理高度并大于文字。
        int largest = CandidateScalePolicy.dimension(32, CandidateScalePolicy.MAX_PERCENT);
        check(CandidateScalePolicy.expandedTextSize(largest) == 60);
        check(CandidateScalePolicy.expandedCellHeight(base, largest, CandidateScalePolicy.MAX_PERCENT) == 156);
        check(CandidateScalePolicy.expandedTextSize(1) >= 1);
    }
}
