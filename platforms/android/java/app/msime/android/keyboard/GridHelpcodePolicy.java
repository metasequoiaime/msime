package app.msime.android;

/**
 * 十四／十八键网格辅助码在 Android 键盘上的规则。辅助码的状态、匹配和清理都在引擎里（`msime_client_grid_helpcode`），这里只决定按键送哪个操作、怎么画和读。
 *
 * <p>上滑「分词」键放下分隔符；之后字母键送整组（{@link #GROUP}），十八键双拼的横滑送确定字母（{@link #EXACT}，小写）；引擎此时不再收 `grid_key`。长按字母弹层照旧结束组字再上屏字母，不进辅助码槽。视图的 `grid_helpcode_display` 非空就是放下了分隔符。
 */
public final class GridHelpcodePolicy {
    /** `msime_client_grid_helpcode` 的操作号。 */
    public static final int BEGIN = 0;
    public static final int EXACT = 1;
    public static final int CLEAR = 2;
    public static final int GROUP = 3;
    /** 字母键照旧送 `grid_key`。 */
    public static final int NONE = -1;
    /** 上滑分词键的距离（dp），与十八键横滑相同。 */
    public static final float ENTRY_THRESHOLD_DP = EighteenKeySwipePolicy.THRESHOLD_DP;
    /** 能用网格辅助码时分词键右上角的角标。 */
    public static final String CORNER_HINT = "辅";

    private GridHelpcodePolicy() {}

    /** 字母键送哪个操作：没有分隔符时 {@link #NONE}；有分隔符时大写（十八键双拼横滑的确定字母标记）送 {@link #EXACT}，其余送 {@link #GROUP}，字母一律转成小写送出。 */
    public static int letterOp(String display, char letter) {
        if (display == null || display.isEmpty()) return NONE;
        return letter >= 'A' && letter <= 'Z' ? EXACT : GROUP;
    }

    /** 分词键这次触摸是不是上滑：向上（`dy` 为负）超过阈值，且纵向明显多于横向。 */
    public static boolean entrySwipe(float dx, float dy) {
        if (!Float.isFinite(dx) || !Float.isFinite(dy)) return false;
        return -dy > ENTRY_THRESHOLD_DP && Math.abs(dy) > 1.5f * Math.abs(dx);
    }

    /** 明确选了一个字面（长按字母、快捷符号）之前怎样结束组字：有分隔符而没有候选时取消（命令 3），否则照旧原样上屏（命令 2，14／18 键有候选时就是按首选结束）。否则原样上屏会把组码字母留在字面前面。 */
    public static int literalPrecommitCommand(String display, int candidates) {
        return display != null && !display.isEmpty() && candidates == 0 ? 3 : 2;
    }

    /** 系统返回键先清辅助码：放下了分隔符、键盘显示着、没有面板盖着；下一次返回再照旧收起键盘。 */
    public static boolean backClears(String display, boolean inputShown, boolean panelOpen) {
        return display != null && !display.isEmpty() && inputShown && !panelOpen;
    }

    /** 分词键的角标：这次组字能用网格辅助码时画「辅」。 */
    public static String cornerHint(boolean available) {
        return available ? CORNER_HINT : "";
    }

    /** 分词键的读屏描述（不含「按键 」前缀）：能用网格辅助码时说明上滑。 */
    public static String separatorDescription(String base, boolean available) {
        return available ? base + "；上滑输入辅助码" : base;
    }

    /** 放下分隔符后拼音选择区里的是辅助码的整串解释，读屏不说成拼音。 */
    public static String choiceDescription(String choice) {
        return "选择辅助码 " + choice;
    }

    /** 读音行：读音后面接引擎给的辅助码显示（`|Q[ER]`）。 */
    public static String reading(String reading, String display) {
        if (display == null || display.isEmpty()) return reading;
        return (reading == null ? "" : reading) + display;
    }
}
