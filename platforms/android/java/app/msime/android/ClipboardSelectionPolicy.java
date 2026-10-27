package app.msime.android;

/** 输入法面板多选剪贴板记录的边界；导出和删除共用同一上限。 */
public final class ClipboardSelectionPolicy {
    public static final int MAX_SELECTIONS = 20;

    private ClipboardSelectionPolicy() {}

    public static boolean canSelect(int selectedCount) {
        return selectedCount >= 0 && selectedCount < MAX_SELECTIONS;
    }
}
