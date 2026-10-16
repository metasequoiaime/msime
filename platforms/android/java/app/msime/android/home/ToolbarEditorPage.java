package app.msime.android.home;

/** 键盘长按和功能菜单共用的工具栏编辑入口，复用键盘页的显隐、排序和保存逻辑。 */
public final class ToolbarEditorPage extends KeyboardOptionsPage {
    @Override protected String initialSection() { return "工具栏"; }
}
