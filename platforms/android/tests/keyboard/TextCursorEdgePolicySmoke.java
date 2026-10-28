import android.text.InputType;
import app.msime.android.TextCursorEdgePolicy;
import java.util.function.IntFunction;

/** 方向键在文档首尾的拦截（#10141）：只用合成文字，按编辑器真实的读写约定回放。 */
public final class TextCursorEdgePolicySmoke {
    private static final int TEXT = InputType.TYPE_CLASS_TEXT;
    private static final int PASSWORD = InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD;
    private static int reads;

    public static void main(String[] args) {
        caretAtEdges();
        clampsToRemainingText();
        diagonalSharesOneSide();
        selections();
        editorsLeftAlone();
        passwordFieldsAreNotRead();
        System.out.println("Android text cursor edges: edges, clamping, selections and unreadable editors passed");
    }

    private static void caretAtEdges() {
        // 光标在开头：往左、往上都不发，往右、往下照发。
        plan(-1, 0, false, 0, 0, TEXT, "", "abc", 0, 0, "开头不再左移");
        plan(0, -1, false, 0, 0, TEXT, "", "abc", 0, 0, "开头不再上移");
        plan(1, 0, false, 0, 0, TEXT, "", "abc", 1, 0, "开头仍可右移");
        plan(0, 1, false, 0, 0, TEXT, "", "abc", 0, 1, "开头仍可下移");
        // 光标在末尾：往右、往下都不发。
        plan(1, 0, false, 3, 3, TEXT, "abc", "", 0, 0, "末尾不再右移");
        plan(0, 1, false, 3, 3, TEXT, "abc", "", 0, 0, "末尾不再下移");
        plan(-1, -1, false, 3, 3, TEXT, "abc", "", -1, -1, "末尾仍可往回");
        // 选区回报还没到（-1）时照样按读到的文字拦。
        plan(1, 0, false, -1, -1, TEXT, "abc", "", 0, 0, "选区未知时按回读拦");
    }

    private static void clampsToRemainingText() {
        plan(-5, 0, false, 2, 2, TEXT, "ab", "", -2, 0, "横向按剩余字数截断");
        // 横向按字素数：代理对和组合附加符都只算一步。
        plan(-5, 0, false, 3, 3, TEXT, "a\uD83D\uDE00", "", -2, 0, "代理对算一个字");
        plan(-5, 0, false, 2, 2, TEXT, "e\u0301", "", -1, 0, "组合附加符跟前一个字算一个字");
        // 纵向按换行数：两个换行最多上移两行；同一行里只发一下（光标先到行首或文档开头）。
        plan(0, -5, false, 14, 14, TEXT, "line1\nline2\nab", "", 0, -2, "纵向按换行数截断");
        plan(0, -3, false, 3, 3, TEXT, "abc", "", 0, -1, "首行只上移一下");
        plan(0, 4, false, 0, 0, TEXT, "", "a\nb\nc\nd", 0, 3, "纵向向下按换行数截断");
        // 读取量按步数放大，有上限。
        IntFunction<CharSequence> limitCheck = limit -> {
            check(limit >= 256 && limit <= 1024, "回读长度在上下限内：" + limit);
            return "abc";
        };
        TextCursorEdgePolicy.plan(-64, -64, false, 3, 3, TEXT, limitCheck, limitCheck);
    }

    private static void diagonalSharesOneSide() {
        // 斜向同一侧：横向走掉的文字纵向不能再用，否则一批里先到开头再多发一个上键。
        plan(-2, -1, false, 2, 2, TEXT, "ab", "", -2, 0, "横向走到开头后不再上移");
        plan(-1, -3, false, 3, 3, TEXT, "a\nb", "", -1, -1, "横向走掉一个字后按剩下的换行算");
        plan(1, 2, false, 0, 0, TEXT, "", "a\nb\nc", 1, 2, "向右下按剩下的文字算");
        // 两侧不同：各按各的一侧。
        plan(-1, 1, false, 2, 2, TEXT, "ab", "", -1, 0, "左移可以、末尾不再下移");
    }

    private static void selections() {
        // 不在选择模式时，有选区的第一下只收起选区：在开头也要发这一下，之后按选区外的文字算。
        plan(-1, 0, false, 0, 3, TEXT, "", "d", -1, 0, "选区在开头时左键收起选区");
        plan(-3, 0, false, 0, 3, TEXT, "", "d", -1, 0, "收起后不再越过开头");
        plan(-4, 0, false, 1, 3, TEXT, "a", "d", -2, 0, "收起加剩余字数");
        // 整段选中时上键会把键交还系统，开头一侧没有文字就连收起也不发。
        plan(0, -1, false, 0, 3, TEXT, "", "", 0, 0, "全选后上键不发");
        plan(0, -2, false, 1, 3, TEXT, "a", "", 0, -1, "上键只发收起那一下");
        // 选择模式下活动端朝锚点收缩，不读也不拦。
        plan(-3, 0, true, 0, 3, TEXT, null, null, -3, 0, "活动端朝锚点收缩时不拦");
        plan(3, 0, true, 3, 0, TEXT, null, null, 3, 0, "活动端从左朝锚点收缩时不拦");
        // 活动端在外侧时按外侧文字拦。
        plan(-1, 0, true, 3, 0, TEXT, "", "d", 0, 0, "活动端已在开头不再延伸");
        plan(2, 0, true, 0, 3, TEXT, "", "d", 1, 0, "活动端向右延伸按剩余字数截断");
    }

    private static void editorsLeftAlone() {
        // TYPE_NULL（终端一类）完全不读，按原样发。
        reads = 0;
        plan(-3, 2, false, 0, 0, InputType.TYPE_NULL, "", "", -3, 2, "TYPE_NULL 原样发");
        check(reads == 0, "TYPE_NULL 不读文字");
        // 读不到文字的编辑器按原样发。
        plan(-3, 0, false, 0, 0, TEXT, null, null, -3, 0, "读不到时原样发");
        // 整个文档读出来都是空的（自己处理方向键、不维护文字的编辑器），按原样发。
        plan(-2, -1, false, 0, 0, TEXT, "", "", -2, -1, "空文档原样发");
        plan(1, 0, false, -1, -1, TEXT, "", "", 1, 0, "空文档且选区未知时原样发");
    }

    private static void passwordFieldsAreNotRead() {
        reads = 0;
        plan(-1, 0, false, 0, 0, PASSWORD, "x", "x", 0, 0, "密码框开头不再左移");
        plan(0, -1, false, 0, 0, PASSWORD, "x", "x", 0, 0, "密码框开头不再上移");
        plan(-5, 0, false, 2, 2, PASSWORD, "x", "x", -2, 0, "密码框按报来的偏移截断");
        plan(0, -3, false, 2, 2, PASSWORD, "x", "x", 0, -1, "密码框上移只发一下");
        plan(3, 1, false, 2, 2, PASSWORD, "x", "x", 3, 1, "密码框末尾一侧不知道长度，原样发");
        plan(-3, 0, false, -1, -1, PASSWORD, "x", "x", -3, 0, "密码框选区未知时原样发");
        plan(-1, 0, false, 0, 4, PASSWORD, "x", "x", -1, 0, "密码框选区在开头时左键收起");
        check(reads == 0, "密码框一次也不读文字");
    }

    private static void plan(int horizontal, int vertical, boolean selecting, int start, int end, int inputType,
            String before, String after, int expectedHorizontal, int expectedVertical, String message) {
        TextCursorEdgePolicy.Plan plan = TextCursorEdgePolicy.plan(horizontal, vertical, selecting, start, end,
            inputType, reader(before, false), reader(after, true));
        check(plan.horizontal() == expectedHorizontal && plan.vertical() == expectedVertical,
            message + "：得到 " + plan);
    }

    /** 编辑器按请求长度从靠近光标的一端截取，与 getTextBeforeCursor / getTextAfterCursor 一致。 */
    private static IntFunction<CharSequence> reader(String text, boolean after) {
        return limit -> {
            reads++;
            if (text == null || text.length() <= limit) return text;
            return after ? text.substring(0, limit) : text.substring(text.length() - limit);
        };
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
