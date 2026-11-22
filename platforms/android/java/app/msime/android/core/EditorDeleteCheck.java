package app.msime.android;

/**
 * 交给编辑器的退格确认之后，对比发出前后读到的光标前文字，判断编辑器删掉的是不是一个普通码位。不依赖 Android，便于在 JVM 上测试。
 *
 * <p>退格的左右滑恢复（{@link BackspaceScrub}）只能恢复确认是纯文字的部分：按键事件可能让应用整块删掉它自己的对象（微信的 `[微笑]`、QQ 的表情和 @），输入法读到的只是文字，原样写回去得到的是假的纯文字。所以只有恰好少了最后一个码位、而且那个码位不是对象占位符（U+FFFC）时，才把这次删除并进按压的恢复记录；别的情况一律作废恢复。
 */
public final class EditorDeleteCheck {
    private EditorDeleteCheck() {}

    /**
     * 光标前最后一个码位是拉丁类组合附加符号（U+0300–U+036F 组合附加符号，U+1AB0–U+1AFF 及 U+1DC0–U+1DFF 扩展与补充）。
     *
     * <p>这是交给编辑器的唯一例外：分解形式的「e + U+0301」在系统 `EditText` 里按键退格只删掉附加符号，要按两次；输入法看得到完整的字母，按字素直接删一次删完（#11444 要求的行为）。应用的对象（`[微笑]`、`@名字`、U+FFFC）不会以附加符号结尾，不受影响；印地文、泰文等文字的元音和声调符号不在这几段里，仍交给编辑器。
     */
    public static boolean endsWithLatinCombiningMark(CharSequence before) {
        if (before == null || before.length() == 0) return false;
        int last = Character.codePointBefore(before, before.length());
        return last >= 0x0300 && last <= 0x036F || last >= 0x1AB0 && last <= 0x1AFF || last >= 0x1DC0 && last <= 0x1DFF;
    }

    /**
     * `before` 和 `after` 是发出退格前后各读一次的光标前文字，长度上限都是 `window`（读不全时只有最后 `window` 个单元）。恰好删掉了 `before` 的最后一个码位、它又不是 U+FFFC 时返回 true。
     */
    public static boolean onePlainCodePointRemoved(CharSequence before, CharSequence after, int window) {
        if (before == null || after == null || before.length() == 0) return false;
        String previous = before.toString();
        String current = after.toString();
        int last = previous.codePointBefore(previous.length());
        if (last == 0xFFFC) return false;
        String expected = previous.substring(0, previous.length() - Character.charCount(last));
        // 读满了窗口时更早的文字没读到，删掉一个码位后窗口往前多带进来一个单元，只能核对结尾。
        return previous.length() < window ? current.equals(expected) : current.endsWith(expected);
    }
}
