import app.msime.android.TranslationReturnTarget;

/** 翻译窗口可以重建输入连接，不能凭应用名相同就向新输入框写入。 */
public final class TranslationReturnTargetSmoke {
    public static void main(String[] args) {
        TranslationReturnTarget target = target("synthetic.app", 7, 1, 2, 2, "ab", null, "cd");
        check(target.same(target("synthetic.app", 7, 1, 2, 2, "ab", null, "cd")), "原编辑器恢复允许插入");
        check(!target.same(null), "连接未恢复禁止插入");
        check(!target.same(target("other.app", 7, 1, 2, 2, "ab", null, "cd")), "换应用拒绝");
        check(!target.same(target("synthetic.app", 8, 1, 2, 2, "ab", null, "cd")), "换输入框拒绝");
        check(!target.same(target("synthetic.app", 7, 2, 2, 2, "ab", null, "cd")), "输入类型改变拒绝");
        check(!target.same(target("synthetic.app", 7, 1, 1, 2, "ab", null, "cd")), "移动选区拒绝");
        check(!target.same(target("synthetic.app", 7, 1, 2, 3, "ab", null, "cd")), "扩大选区拒绝");
        check(!target.same(target("synthetic.app", 7, 1, 2, 2, "xy", null, "cd")), "前文变化拒绝");
        check(!target.same(target("synthetic.app", 7, 1, 2, 2, "ab", "x", "cd")), "选择文字变化拒绝");
        check(!target.same(target("synthetic.app", 7, 1, 2, 2, "ab", null, "xy")), "后文变化拒绝");
        System.out.println("translation return target passed");
    }
    private static TranslationReturnTarget target(String owner, int field, int type, int start, int end,
            String before, String selected, String after) {
        return new TranslationReturnTarget(owner, field, type, start, end, before, selected, after);
    }
    private static void check(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
