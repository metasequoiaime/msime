import app.msime.android.BackspaceSwipePolicy;
import app.msime.android.QuickDeleteUndo;

/** 只用合成内容覆盖分段备份、拒写、容量和撤回上下文边界。 */
public final class QuickDeleteUndoSmoke {
    public static void main(String[] args) {
        String original = "合成😀".repeat(3000);
        StringBuilder document = new StringBuilder(original);
        QuickDeleteUndo.Capture capture = new QuickDeleteUndo.Capture();
        long deleted = BackspaceSwipePolicy.clearBeforeCursor(editor(document, false), capture);
        check(deleted == original.length() && capture.text().equals(original), "分段倒序恢复与代理对");
        QuickDeleteUndo.Capture rejected = new QuickDeleteUndo.Capture();
        document.append("合成拒写");
        check(BackspaceSwipePolicy.clearBeforeCursor(editor(document, true), rejected) == 0
            && rejected.text().isEmpty() && document.toString().equals("合成拒写"), "拒删不能进入备份");
        QuickDeleteUndo.Capture selection = new QuickDeleteUndo.Capture();
        selection.deleted("选中");
        selection.deleted("前文");
        check(selection.text().equals("前文选中"), "选区恢复在前文之后");
        selection.deleted("x".repeat(QuickDeleteUndo.MAX_UNITS));
        check(selection.text().isEmpty(), "超限不保留半份备份");
        Object target = new Object();
        QuickDeleteUndo undo = new QuickDeleteUndo(target, 10, original, "前", "后", 100);
        check(undo.matches(target, 10, "前", "后", 101), "原输入框原位置可撤回");
        check(!undo.matches(new Object(), 10, "前", "后", 101), "切换连接拒绝");
        check(!undo.matches(target, 9, "前", "后", 101), "相同文本其他位置拒绝");
        check(!undo.matches(target, 10, "改", "后", 101), "改动前文拒绝");
        check(!undo.matches(target, 10, "前", "改", 101), "改动后文拒绝");
        check(!undo.matches(target, 10, null, "后", 101), "读取失败拒绝");
        check(!undo.matches(target, 10, "前", "后", 100 + QuickDeleteUndo.LIFETIME_MS), "超时拒绝");
        check(!undo.matches(target, 10, "前", "后", 99), "时钟倒退拒绝");
        check(undo.atSelection(10, 10) && !undo.atSelection(10, 11), "选区必须折叠在原位");
        QuickDeleteUndo bounded = new QuickDeleteUndo(target, 0, "合成", "x".repeat(1000), "y".repeat(1000), 0);
        check(bounded.matches(target, 0, "x".repeat(64), "y".repeat(64), 1), "不配合长度的编辑器仍只保留短上下文");
        check(BackspaceSwipePolicy.undoPhase(131, 100, 2) == BackspaceSwipePolicy.Phase.REVEALED,
            "不足16dp不待命");
        check(BackspaceSwipePolicy.undoPhase(132, 100, 2) == BackspaceSwipePolicy.Phase.ARMED,
            "下滑16dp待命");
        check(BackspaceSwipePolicy.undoPhase(99, 100, 2) == BackspaceSwipePolicy.Phase.REVEALED,
            "滑回可取消");
        var box = BackspaceSwipePolicy.undoBox(900, 300, 1000, 400, 1080, 600, 2);
        check(box.top() > 400 && box.bottom() <= 592 && box.right() <= 1072, "撤回框在键下方且不越界");
        System.out.println("Android quick-delete undo passed");
    }

    private static BackspaceSwipePolicy.Editor editor(StringBuilder text, boolean reject) {
        return new BackspaceSwipePolicy.Editor() {
            @Override public CharSequence textBeforeCursor(int length) {
                return text.substring(Math.max(0, text.length() - Math.min(length, 71)));
            }
            @Override public boolean deleteBeforeCursor(int length) {
                if (reject) return false;
                text.setLength(text.length() - length);
                return true;
            }
        };
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
