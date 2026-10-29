package app.msime.android;

/**
 * 大屏单手模式空出来那一侧放什么（#10203，共享偏好 `touch_one_handed_side_panel`）：第一版只有数字键盘，见 `.agents/notes/implemented/feature/2026-10-29-tablet-one-hand-side-number-pad.md`。
 *
 * <p>这里只做不依赖视图的判断：偏好怎么读、什么时候画。面板怎么建、摆在哪由 `ImeSidePad` 和 `ImeFrame.applyOneHanded` 负责，宽度由 {@link KeyboardFormFactorPolicy#oneHandedSidePadWidthDp} 给出。
 */
public final class OneHandSidePadPolicy {
    /** 共享偏好的键，与 client-core 的 `touch_one_handed_side_panel` 相同。 */
    public static final String PREFERENCE_KEY = "touch_one_handed_side_panel";

    private OneHandSidePadPolicy() {}

    /** 空出来那一侧放的面板。以后的文本编辑面板（`text_edit`）这一版读作 {@link #OFF}。 */
    public enum Panel {
        OFF("off"),
        NUMBER_PAD("number_pad");

        public final String wire;

        Panel(String wire) {
            this.wire = wire;
        }
    }

    /** 读偏好里的写法；缺键、不是字符串或不认识的写法都按 {@link Panel#OFF}，与 client-core 读文档时的宽松读取一致。 */
    public static Panel parse(String value) {
        return Panel.NUMBER_PAD.wire.equals(value) ? Panel.NUMBER_PAD : Panel.OFF;
    }

    /**
     * 侧边数字键盘此刻画不画，五条全满足才画。
     *
     * @param panel 偏好读出的面板
     * @param effectiveMode 实际生效的单手取值（`ImeFrame.effectiveOneHanded`：分离式、浮动时已是 `off`）
     * @param widthDp {@link KeyboardFormFactorPolicy#oneHandedSidePadWidthDp} 的结果，0 表示手机或放不下
     * @param drawnLayout 键行建出来时的触屏布局（英文和本地模式时是 26 键）
     * @param layer 键盘层
     * @param numberPadDrawn 主键区此刻画的是数字九宫格
     */
    public static boolean shown(Panel panel, String effectiveMode, int widthDp, int drawnLayout,
                                KeyboardLayout.Layer layer, boolean numberPadDrawn) {
        return panel == Panel.NUMBER_PAD
            && ("left".equals(effectiveMode) || "right".equals(effectiveMode))
            && widthDp > 0
            && nineKeyLetters(drawnLayout, layer, numberPadDrawn);
    }

    /** 主键区画的是九键一族（拼音九键、九键双拼、笔画）的字母层；九键双拼与拼音九键是同一个触屏布局。注音九键、日文假名九键、手写、14/18 键、26 键和 123 / #+= 层都不算。 */
    static boolean nineKeyLetters(int drawnLayout, KeyboardLayout.Layer layer, boolean numberPadDrawn) {
        return layer == KeyboardLayout.Layer.LETTERS && !numberPadDrawn
            && (drawnLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT || drawnLayout == KeyboardLayout.STROKE_LAYOUT);
    }
}
