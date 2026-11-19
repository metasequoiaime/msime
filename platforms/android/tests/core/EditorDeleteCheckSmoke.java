import app.msime.android.EditorDeleteCheck;

public final class EditorDeleteCheckSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 普通文字删掉最后一个码位：并进恢复记录。
        check(EditorDeleteCheck.onePlainCodePointRemoved("前后", "前", 1024));
        // 代理对按一个码位算。
        check(EditorDeleteCheck.onePlainCodePointRemoved("甲🙂", "甲", 1024));
        // 应用整块删掉了自己的对象（微信 `[Smile]`、QQ 的 @）：不是一个码位，作废恢复。
        check(!EditorDeleteCheck.onePlainCodePointRemoved("你好[Smile]", "你好", 1024));
        check(!EditorDeleteCheck.onePlainCodePointRemoved("@calciTEA ", "", 1024));
        // 删掉的是对象占位符：即使只少一个单元也不能当文字恢复。
        check(!EditorDeleteCheck.onePlainCodePointRemoved("前￼", "前", 1024));
        // 编辑器没删（不处理按键）、删多了、读不到：都不并进。
        check(!EditorDeleteCheck.onePlainCodePointRemoved("前后", "前后", 1024));
        check(!EditorDeleteCheck.onePlainCodePointRemoved("前后", "", 1024));
        check(!EditorDeleteCheck.onePlainCodePointRemoved("前后", null, 1024));
        check(!EditorDeleteCheck.onePlainCodePointRemoved(null, "前", 1024));
        check(!EditorDeleteCheck.onePlainCodePointRemoved("", "", 1024));
        // 删除前读满了窗口：删掉一个码位后窗口多带进一个更早的单元，只核对结尾。
        check(EditorDeleteCheck.onePlainCodePointRemoved("bcd", "abc", 3));
        check(!EditorDeleteCheck.onePlainCodePointRemoved("bcd", "abd", 3));
        // 没读满窗口时必须完全相同：前面多出来的文字说明不是这次删除。
        check(!EditorDeleteCheck.onePlainCodePointRemoved("bcd", "abc", 4));
        // 拉丁类组合附加符号：分解形式的 é、多个附加符号叠加、扩展段，都按整字直接删。
        check(EditorDeleteCheck.endsWithLatinCombiningMark("cafe\u0301"));
        check(EditorDeleteCheck.endsWithLatinCombiningMark("a\u0308\u0301"));
        check(EditorDeleteCheck.endsWithLatinCombiningMark("x\u1AB0"));
        check(EditorDeleteCheck.endsWithLatinCombiningMark("x\u1DC0"));
        // 组合形式的 é、表情的键帽和变体选择符、印地文元音符号、应用对象：交给编辑器。
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("caf\u00e9"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("1\ufe0f\u20e3"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("\u2764\ufe0f"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("\u0915\u093f"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("[Smile]"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("@name\u2005"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark("\ufffc"));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark(""));
        check(!EditorDeleteCheck.endsWithLatinCombiningMark(null));
        System.out.println("EditorDeleteCheck smoke passed");
    }
}
