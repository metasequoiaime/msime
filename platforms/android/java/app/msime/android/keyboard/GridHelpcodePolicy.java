package app.msime.android;

/**
 * 十四／十八键网格辅助码在 Android 键盘上的规则。辅助码的状态、匹配和清理都在引擎里（`msime_client_grid_helpcode`），这里只决定按键送哪个操作、怎么画和读。
 *
 * <p>上滑「分词」键放下分隔符；之后字母键送整组（{@link #GROUP}），14/18 键两字母键的横滑送确定字母（{@link #EXACT}，小写）；引擎此时不再收 `grid_key`。长按字母弹层照旧结束组字再上屏字母，不进辅助码槽。视图的 `grid_helpcode_display` 非空就是放下了分隔符。
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

    /** 字母键送哪个操作：没有分隔符时 {@link #NONE}；有分隔符时大写（14/18 键横滑的确定字母标记）送 {@link #EXACT}，其余送 {@link #GROUP}，字母一律转成小写送出。 */
    public static int letterOp(String display, char letter) {
        if (display == null || display.isEmpty()) return NONE;
        return letter >= 'A' && letter <= 'Z' ? EXACT : GROUP;
    }

    /** 分词键这次触摸是不是上滑：向上（`dy` 为负）超过阈值，且纵向明显多于横向。 */
    public static boolean entrySwipe(float dx, float dy) {
        if (!Float.isFinite(dx) || !Float.isFinite(dy)) return false;
        return -dy > ENTRY_THRESHOLD_DP && Math.abs(dy) > 1.5f * Math.abs(dx);
    }

    /** 回车留着组字、什么也不做：至少有一槽辅助码而筛空，与引擎吞掉空格、标点、数字的条件相同（只有分隔符时显示就是 `|`，候选没有筛过，回车照旧原样上屏）。回车和空格、标点一样是用户结束一个词的键；引擎此时的原样上屏会丢掉组字（给插入别的内容之前结束组字用），回车不该丢。 */
    public static boolean holdsEnter(String display, int candidates) {
        return constrains(display) && candidates == 0;
    }

    /** 显示里有分隔符之后的辅助码，候选是筛过的（引擎的 `grid_helpcode_constrained`）。 */
    static boolean constrains(String display) {
        return display != null && display.length() > 1;
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
