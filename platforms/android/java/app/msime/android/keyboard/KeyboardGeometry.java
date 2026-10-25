package app.msime.android;

/** Apple-compatible touch-keyboard geometry constants; input algorithms remain in Engine. */
public final class KeyboardGeometry {
    /** 所有布局的每一行键高，包括底行（123、中/英、空格、换行那一行）：26 键字母行、九键网格、笔画、手写区、大千四行整块和底行都按它排，每行另加一份行距，彼此切换时键盘总高不变；竖屏手写区例外，多一行（{@link #handwritingRows}）。默认行距下四行共 4 × (52 + 7) = 236 dp，与原来三行 56 dp 键行加 46 dp 不带行距的底栏（235 dp）差 1 dp；底行和上面的键一样高，与 iOS、HarmonyOS 相同（#6354、#6472）。 */
    public static final int KEY_ROW_HEIGHT_DP = 52;
    /** Fixed candidate/shortcut row; swapping its contents must not move the key rows. */
    public static final int CANDIDATE_ROW_HEIGHT_DP = 48;
    public static final int NINE_KEY_HEIGHT_DP = 180;
    public static final int HANDWRITING_BODY_HEIGHT_DP = 220;
    /** 竖屏手写区占几行键高：书写区铺满键区宽度，比其他布局的三行键多一行，键盘因此高一行（键高加一份行距）。 */
    public static final int HANDWRITING_PORTRAIT_ROWS = 4;
    /** 横屏手写区仍是三行：横屏窗口本来就矮，再加一行会挤掉编辑区。iOS 与 HarmonyOS 用同一条规则。 */
    public static final int HANDWRITING_LANDSCAPE_ROWS = 3;

    /** 手写区占的行数（{@link #HANDWRITING_PORTRAIT_ROWS} 或 {@link #HANDWRITING_LANDSCAPE_ROWS}）。 */
    public static int handwritingRows(boolean landscape) {
        return landscape ? HANDWRITING_LANDSCAPE_ROWS : HANDWRITING_PORTRAIT_ROWS;
    }

    /** 新设计的标准键高、工具栏高度和键盘外边距/间距。 */
    public static final int DESIGN_KEY_HEIGHT_DP = 46;
    public static final int DESIGN_TOOLBAR_ROW_HEIGHT_DP = 50;
    public static final int DESIGN_PADDING_TOP_DP = 8;
    public static final int DESIGN_PADDING_HORIZONTAL_DP = 6;
    public static final int DESIGN_PADDING_BOTTOM_DP = 6;
    public static final int DESIGN_KEY_GAP_DP = 5;
    public static final int DESIGN_ROW_GAP_DP = 8;


    private KeyboardGeometry() { }

}
