import app.msime.android.BackspaceScrub;

/** 用独立合成编辑器检查连续删除/恢复与拒写边界，避免依赖 Android 桩。 */
public final class BackspaceScrubSmoke {
    private BackspaceScrubSmoke() {}

    public static void main(String[] args) {
        Editor editor = new Editor("合成😀甲乙", "后文");
        BackspaceScrub scrub = new BackspaceScrub(editor, editor.read());
        editor.delete(1);
        scrub.ordinaryDelete("甲乙");
        check(scrub.move(editor, 4) && editor.before.equals("合"), "左滑包含按下的一字");
        check(scrub.move(editor, 2) && editor.before.equals("合成😀"), "右滑按原顺序恢复且不拆代理对");
        check(scrub.move(editor, 0) && editor.before.equals("合成😀甲乙"), "回到起点恢复全部");
        check(editor.after.equals("后文"), "光标后文不动");
        check(scrub.move(editor, 999) && editor.before.isEmpty(), "不能越过原文起点");
        check(scrub.move(editor, -1) && editor.before.equals("合成😀甲乙"), "不能恢复到原文之外");

        for (int failure = 0; failure < 7; failure++) {
            Editor changed = new Editor("合成甲乙", "后");
            BackspaceScrub bounded = new BackspaceScrub(changed, changed.read());
            check(bounded.move(changed, 2), "先删两字");
            switch (failure) {
                case 0 -> changed.identity = new Object();
                case 1 -> changed.cursorOffset = 1;
                case 2 -> changed.before = "改文";
                case 3 -> changed.after = "改";
                case 4 -> changed.readable = false;
                case 5 -> changed.reject = true;
                case 6 -> changed.ignore = true;
                default -> throw new AssertionError();
            }
            check(!bounded.move(changed, 0), "连接、位置、前后文、拒写或虚假成功必须终止");
            changed.identity = changed;
            changed.readable = true;
            changed.reject = false;
            changed.ignore = false;
            int writes = changed.writes;
            check(!bounded.move(changed, 0) && changed.writes == writes, "失效后不盲目重试");
        }

        Editor repeated = new Editor("甲乙😀", "");
        BackspaceScrub held = new BackspaceScrub(repeated, repeated.read());
        repeated.delete(2);
        held.ordinaryDelete("😀");
        repeated.delete(1);
        held.ordinaryDelete("甲乙");
        check(held.move(repeated, 0) && repeated.before.equals("甲乙😀"), "长按连删也能在同次恢复");
        BackspaceScrub wrong = new BackspaceScrub(repeated, repeated.read());
        wrong.ordinaryDelete("变更");
        check(!wrong.available(), "普通删除文字不符终止");

        Editor large = new Editor("外".repeat(100) + "😀".repeat(3000), "尾");
        String original = large.before;
        BackspaceScrub limit = new BackspaceScrub(large, large.read());
        check(limit.count() == 2048, "只保留4096单元码位");
        check(limit.move(large, limit.count()) && large.before.length() == original.length() - 4096,
            "有界删除保留前侧校验上下文");
        check(limit.move(large, 0) && large.before.equals(original), "有界大文本完整恢复");
        check(!BackspaceScrub.starts(7, 0, 1) && !BackspaceScrub.starts(16, 14, 1)
            && BackspaceScrub.starts(16, 2, 2), "触发阈值和方向锁");
        check(BackspaceScrub.deletionCount(0, 300, 100, 8, 5) == 0
            && BackspaceScrub.deletionCount(8, 300, 100, 8, 5) == 5
            && BackspaceScrub.deletionCount(300, 300, 100, 8, 5) == 100,
            "起点、连删激活点和左边缘映射");
        check(BackspaceScrub.deletionCount(150, 300, 100, 8, 1)
            == BackspaceScrub.deletionCount(300, 600, 100, 16, 1), "屏幕尺寸归一");
        System.out.println("Android backspace scrub passed");
    }

    private static final class Editor implements BackspaceScrub.Editor {
        private Object identity = this;
        private String before;
        private String after;
        private int cursorOffset;
        private int writes;
        private boolean readable = true;
        private boolean reject;
        private boolean ignore;

        Editor(String before, String after) { this.before = before; this.after = after; }
        @Override public Object target() { return identity; }
        @Override public BackspaceScrub.Frame read() {
            return readable ? new BackspaceScrub.Frame(before.length() + cursorOffset,
                before.substring(Math.max(0, before.length() - 4160)), after) : null;
        }
        @Override public boolean delete(int units) {
            writes++;
            if (reject) return false;
            if (!ignore) before = before.substring(0, before.length() - units);
            return true;
        }
        @Override public boolean restore(String text) {
            writes++;
            if (reject) return false;
            if (!ignore) before += text;
            return true;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
