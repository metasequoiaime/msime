package app.msime.android;

/** Android 空闲工具栏的独立尺寸；未显式设置时由调用方继续沿用候选栏高度，按钮和图标随行高缩小（{@link #followedButtonHeight}）。 */
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

    /** 按钮画图标和选中底块时用的比例：跟随候选栏缩小时可以低于 {@link #MIN_PERCENT}，上限仍是 {@link #MAX_PERCENT}。 */
    public static int drawPercent(int value) {
        return Math.max(1, Math.min(MAX_PERCENT, value));
    }

    /** 跟随候选栏时按钮的高度：平时是设计的 44 dp（{@code button}），「候选栏大小」调小、这一行（{@code row}）矮于基准行（{@code followedReference}）时取行高，按钮不再伸出这一行被裁掉。{@code designRow} 是同一状态下「候选栏大小」为 100% 时的行高：读音行显示时这一行只有 42 dp 的候选行，44 dp 的按钮本来就略高于它，这是原有设计，100% 时不能因此变矮。三个参数单位相同（键盘传 px，设置页预览传 dp）；行高还没算出来时按设计高度。 */
    public static int followedButtonHeight(int row, int button, int designRow) {
        return row > 0 && row < followedReference(button, designRow) ? row : button;
    }

    /** 跟随候选栏时图标和选中底块的比例：这一行比基准行矮多少，图标就等比缩小多少；不比基准行矮时是 100，所以「候选栏大小」为 100% 时读音行出现或消失，图标都不变。 */
    public static int followedPercent(int row, int button, int designRow) {
        if (button <= 0) return DEFAULT_PERCENT;
        int reference = followedReference(button, designRow);
        return row > 0 && row < reference ? drawPercent(row * 100 / reference) : DEFAULT_PERCENT;
    }

    /** 开始缩小按钮的基准行高：设计按钮高和 100% 时的行高中较矮的一个。 */
    private static int followedReference(int button, int designRow) {
        return designRow > 0 ? Math.min(button, designRow) : button;
    }

    /** 设置页预览用：跟随候选栏时空闲行的默认高度，即 {@code ImeToolbar.READING_ROW_DP} 的读音行加上按「候选栏大小」缩放的 {@code ImeToolbar.CANDIDATE_LINE_DP} 候选行。读音字号放大或多行释义时键盘里的实际行高会更高。 */
    public static int followedHeightDp(int candidateScalePercent) {
        return ImeToolbar.READING_ROW_DP + CandidateScalePolicy.dimension(
            ImeToolbar.CANDIDATE_LINE_DP, CandidateScalePolicy.percent(candidateScalePercent));
    }

    public static int idleSpacerPx(int idleHeight, int composingHeight) {
        return Math.max(0, composingHeight - idleHeight);
    }

    public static int composingSpacerPx(int idleHeight, int composingHeight) {
        return Math.max(0, idleHeight - composingHeight);
    }
}
