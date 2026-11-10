package app.msime.android;

/** 输入法面板多选剪贴板记录的边界；导出和删除共用同一上限，按序号插入另有更小的上限（#10411）。 */
public final class ClipboardSelectionPolicy {
    public static final int MAX_SELECTIONS = 20;
    /** 一次最多按序号插入几条；超过时「插入」置灰，导出和删除照旧可用到 {@link #MAX_SELECTIONS} 条。 */
    public static final int MAX_INSERTIONS = 4;
    /** 带圈数字 ① 的码位；⑳ 正好是第 20 个，和选择上限对得上。 */
    private static final int FIRST_CIRCLED_DIGIT = 0x2460;

    private ClipboardSelectionPolicy() {}

    public static boolean canSelect(int selectedCount) {
        return selectedCount >= 0 && selectedCount < MAX_SELECTIONS;
    }

    /** 选中 1 到 {@link #MAX_INSERTIONS} 条时才能插入。 */
    public static boolean canInsert(int selectedCount) {
        return selectedCount >= 1 && selectedCount <= MAX_INSERTIONS;
    }

    /** 第 `index` 个选中项（从 0 起）的角标：①、②……⑳。 */
    public static String ordinalLabel(int index) {
        if (index < 0 || index >= MAX_SELECTIONS) {
            throw new IllegalArgumentException("ordinal index out of range: " + index);
        }
        return String.valueOf((char) (FIRST_CIRCLED_DIGIT + index));
    }

    /**
     * `text` 在选中集合里的序号（从 0 起），没选中时为 -1。
     *
     * <p>面板用 `LinkedHashSet` 按点选先后保存选中项，取消一项后排在它后面的各项序号自然依次前移，不另外维护序号。
     */
    public static int ordinal(Iterable<String> selected, String text) {
        int index = 0;
        for (String candidate : selected) {
            if (candidate.equals(text)) return index;
            index++;
        }
        return -1;
    }

    /** 按选中顺序把各条首尾相接成一次提交的文字，中间不加分隔符。 */
    public static String insertionText(Iterable<String> texts) {
        StringBuilder joined = new StringBuilder();
        for (String text : texts) {
            if (text != null) joined.append(text);
        }
        return joined.toString();
    }

    /** 选中项超过插入上限、「插入」置灰时给的提示；导出和删除不受这个上限影响。 */
    public static final String INSERTION_LIMIT_HINT = "一次最多插入 " + MAX_INSERTIONS + " 项，导出和删除不受影响";

    /**
     * 刚选中的这一项是否让「插入」从可用变成置灰，这时提示一次 {@link #INSERTION_LIMIT_HINT}。
     *
     * <p>提示不放进顶行的计数里：顶行套在横向滚动里按内容宽度量，计数一长，窄屏上「删除」就被挤到屏外。
     */
    public static boolean exceedsInsertionCap(int selectedCount) {
        return selectedCount == MAX_INSERTIONS + 1;
    }

    /** 选中卡片的读屏前缀；没选中（`index` 为 -1）时为空。 */
    public static String selectedDescription(int index) {
        return index < 0 ? "" : "已选择第 " + (index + 1) + " 项；";
    }
}
