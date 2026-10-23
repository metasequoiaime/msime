import app.msime.android.EditorCopyAll;

/** 只用选区元数据验证编辑器拥有全选和复制权限，以及复制后的位置恢复。 */
public final class EditorCopyAllSmoke {
    private EditorCopyAllSmoke() {}

    public static void main(String[] args) {
        Editor ordinary = new Editor();
        check(EditorCopyAll.copy(ordinary) && ordinary.copies == 1 && ordinary.restores == 1
            && ordinary.selection.equals(new EditorCopyAll.Selection(3, 1, 6)), "复制后保留原反向选区");
        check(ordinary.order.toString().equals("ACR"), "全选、复制、恢复按顺序执行");
        Editor rejected = new Editor();
        rejected.rejectAll = true;
        check(!EditorCopyAll.copy(rejected) && rejected.copies == 0, "拒绝全选不能复制原选区");
        Editor noCopy = new Editor();
        noCopy.rejectCopy = true;
        check(!EditorCopyAll.copy(noCopy) && noCopy.restores == 1, "拒绝复制仍恢复本次全选");
        Editor ignored = new Editor();
        ignored.ignoreAll = true;
        check(!EditorCopyAll.copy(ignored) && ignored.copies == 0, "全选虚假成功不能复制局部选区");
        Editor unknown = new Editor();
        unknown.selection = null;
        check(!EditorCopyAll.copy(unknown) && unknown.order.length() == 0, "未知选区不动编辑器");
        Editor switched = new Editor();
        switched.switchTarget = true;
        check(!EditorCopyAll.copy(switched) && switched.copies == 0 && switched.restores == 0,
            "连接变化不能复制或恢复别的输入框");
        Editor moved = new Editor();
        moved.moveDuringCopy = true;
        check(EditorCopyAll.copy(moved) && moved.restores == 0, "复制回调中的外部移动不被恢复覆盖");
        Editor collapsed = new Editor();
        collapsed.collapseDuringCopy = true;
        check(EditorCopyAll.copy(collapsed) && collapsed.restores == 1, "复制菜单正常折叠到末尾仍恢复原位置");
        Editor selected = new Editor();
        selected.selection = new EditorCopyAll.Selection(0, 6, 6);
        check(EditorCopyAll.copy(selected) && selected.order.toString().equals("CR"), "原本全选无需再次全选");
        System.out.println("Android editor copy-all passed");
    }

    private static final class Editor implements EditorCopyAll.Editor {
        private Object target = this;
        private EditorCopyAll.Selection selection = new EditorCopyAll.Selection(3, 1, 6);
        private final StringBuilder order = new StringBuilder();
        private boolean rejectAll;
        private boolean rejectCopy;
        private boolean ignoreAll;
        private boolean switchTarget;
        private boolean moveDuringCopy;
        private boolean collapseDuringCopy;
        private int copies;
        private int restores;
        @Override public Object target() { return target; }
        @Override public EditorCopyAll.Selection read() { return selection; }
        @Override public boolean selectAll() {
            order.append('A');
            if (rejectAll) return false;
            if (!ignoreAll) selection = new EditorCopyAll.Selection(0, 6, 6);
            if (switchTarget) target = new Object();
            return true;
        }
        @Override public boolean copy() {
            order.append('C');
            copies++;
            if (moveDuringCopy) selection = new EditorCopyAll.Selection(2, 2, 6);
            if (collapseDuringCopy) selection = new EditorCopyAll.Selection(6, 6, 6);
            return !rejectCopy;
        }
        @Override public boolean select(int start, int end) {
            order.append('R');
            restores++;
            selection = new EditorCopyAll.Selection(start, end, 6);
            return true;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
