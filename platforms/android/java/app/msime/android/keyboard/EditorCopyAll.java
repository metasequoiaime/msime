package app.msime.android;

/** 由编辑器执行复制，输入法只处理选区元数据，不能自行把读取的文本写进剪贴板。 */
public final class EditorCopyAll {
    private EditorCopyAll() {}

    /** 仅接受从文档起点提取的完整快照；长度用于核对全选，不保留文本。 */
    public record Selection(int start, int end, int length) {
        public boolean valid() { return start >= 0 && end >= 0 && start <= length && end <= length; }
        public boolean all() { return start == 0 && end == length && length > 0; }
    }

    public interface Editor {
        Object target();
        Selection read();
        boolean selectAll();
        boolean copy();
        boolean select(int start, int end);
    }

    public static boolean copy(Editor editor) {
        Object target = editor.target();
        Selection original = editor.read();
        if (target == null || original == null || !original.valid() || original.length() == 0) return false;
        if (!original.all() && !editor.selectAll()) return false;
        Selection selected = editor.read();
        if (editor.target() != target || selected == null || !selected.all()
                || selected.length() != original.length()) return false;
        boolean copied = editor.copy();
        Selection current = editor.read();
        // TextView 结束复制菜单时可能把全选折叠到末尾，这也是本次操作的正常落点。
        // 其他移动和连接切换都不覆盖，避免抢回应用已经改变的位置。
        boolean copyPosition = selected.equals(current) || current != null && current.length() == selected.length()
            && current.start() == selected.end() && current.end() == selected.end();
        if (editor.target() == target && copyPosition) editor.select(original.start(), original.end());
        return copied;
    }
}
