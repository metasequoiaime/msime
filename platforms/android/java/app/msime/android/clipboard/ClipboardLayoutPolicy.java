package app.msime.android;

/**
 * 剪贴板面板一行排几条（#5642）：本地设置 `platform.android.clipboard_columns`。
 *
 * <p>单列时每条占满一行，长文字能多露出几个字；双列时一行两条，历史多的时候上下翻得少。双列时长按一条打开的操作行跨在那一行两条的下方，被长按的那条以选中样式标出。
 */
public final class ClipboardLayoutPolicy {
    public static final String ONE_COLUMN = "one";
    public static final String TWO_COLUMNS = "two";

    private ClipboardLayoutPolicy() {}

    /** 设置值对应的列数；不认识的值按单列。 */
    public static int columns(String choice) {
        return TWO_COLUMNS.equals(choice) ? 2 : 1;
    }

    /** 第 `index` 条（从 0 起）在第几行。 */
    public static int row(int index, int columns) {
        if (index < 0 || columns < 1) throw new IllegalArgumentException("Bad clipboard grid position");
        return index / columns;
    }

    /** 一共 `count` 条排成几行。 */
    public static int rows(int count, int columns) {
        if (count < 0 || columns < 1) throw new IllegalArgumentException("Bad clipboard grid size");
        return (count + columns - 1) / columns;
    }
}
