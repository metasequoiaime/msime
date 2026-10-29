import android.text.InputType;
import app.msime.android.TextCursorEdgePolicy;

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
        TextCursorEdgePolicy.plan(-64, -64, false, 3, 3, TEXT, new Reads(null, null, null, false) {
            @Override public CharSequence before(int limit) {
                check(limit >= 256 && limit <= 1024, "回读长度在上下限内：" + limit);
                return "abc";
            }
        });
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
        // 整段选中时上下键收起后会把键交还系统，这时连收起也不发；只选了一部分时收起照发（Selection.moveUp / moveDown 返回 true）。
        plan(0, -1, false, 0, 3, TEXT, "", "", 0, 0, "全选后上键不发");
        plan(0, 1, false, 0, 3, TEXT, "", "", 0, 0, "全选后下键不发");
        plan(0, -1, false, 0, 2, TEXT, "", "cd", 0, -1, "选了开头一部分时上键收起选区");
        plan(0, 1, false, 2, 4, TEXT, "ab", "", 0, 1, "选了末尾一部分时下键收起选区");
        plan(0, -2, false, 1, 3, TEXT, "a", "", 0, -1, "上键只发收起那一下");
        // 选区回报已过时（上一批刚收起、回报还没到）：收起那一下要越过边界时按编辑器此刻的选区算。
        planWith(-1, 0, false, 0, 3, TEXT, "", "d", null, false, 0, 0, "选区已收起时开头不再左移");
        planWith(-2, 0, false, 1, 3, TEXT, "a", "d", "", false, -1, 0, "选区已收起时只走剩下的字");
        planWith(0, -1, false, 0, 2, TEXT, "", "cd", null, false, 0, 0, "选区已收起时开头不再上移");
        // 收起那一下碰不到边界时不读选区。
        reads = 0;
        plan(-1, 0, false, 1, 3, TEXT, "a", "d", -1, 0, "离开头还有字时直接收起");
        check(reads == 1, "收起碰不到边界时只读一侧：" + reads);
        // 选择模式下活动端朝锚点收缩：先走选区里的字，再走锚点外侧的文字，到文档边界为止。
        plan(-3, 0, true, 0, 3, TEXT, "", "d", -3, 0, "活动端朝锚点收缩走完选区");
        plan(-5, 0, true, 1, 4, TEXT, "a", "", -4, 0, "收缩越过锚点后到开头为止");
        plan(5, 0, true, 3, 1, TEXT, "a", "d", 3, 0, "活动端从左收缩越过锚点后到末尾为止");
        plan(2, 0, true, 4, 3, TEXT, "abc", "", 1, 0, "锚点在末尾时只走选区里的字");
        plan(-3, 0, true, 0, 3, TEXT, null, null, -3, 0, "读不到时收缩照原样发");
        // 纵向收缩不知道选区里有几行，只按锚点外侧的换行算，至少一步。
        plan(0, -3, true, 5, 9, TEXT, "a\nb\nc", "", 0, -2, "纵向收缩按锚点外侧换行数");
        // 报来 start > end 时方向确定（原生 TextView 原样报锚点和活动端），活动端在外侧时按外侧文字拦。
        plan(-1, 0, true, 3, 0, TEXT, "", "d", 0, 0, "活动端已在开头不再延伸");
        // 报来 start < end 时方向不确定（Chromium WebView 总按 start <= end 报），活动端可能在左端，按可能收缩的上限算。
        plan(2, 0, true, 0, 3, TEXT, "", "d", 2, 0, "方向不确定时向右按选区加外侧文字截断");
        plan(5, 0, true, 0, 3, TEXT, "", "d", 4, 0, "方向不确定时向右到末尾为止");
        // WebView 里从末尾向左选了 3 个字（真实锚点 5、活动端 2）报成 (2,5)：往右收缩不能被末尾拦掉。
        plan(1, 0, true, 2, 5, TEXT, "he", "", 1, 0, "WebView 选区从末尾向左选后仍能向右收缩");
        plan(4, 0, true, 2, 5, TEXT, "he", "", 3, 0, "WebView 选区向右收缩到末尾为止");
        plan(-1, 0, true, 2, 5, TEXT, "he", "", -1, 0, "WebView 选区继续向左延伸");
    }

    private static void editorsLeftAlone() {
        // TYPE_NULL（终端一类）完全不读，按原样发。
        reads = 0;
        plan(-3, 2, false, 0, 0, InputType.TYPE_NULL, "", "", -3, 2, "TYPE_NULL 原样发");
        check(reads == 0, "TYPE_NULL 不读文字");
        // 读不到文字的编辑器按原样发。
        plan(-3, 0, false, 0, 0, TEXT, null, null, -3, 0, "读不到时原样发");
        // 整个文档读出来都是空的，又确认不了是真的空输入框（自己处理方向键、不维护文字的编辑器），按原样发。
        plan(-2, -1, false, 0, 0, TEXT, "", "", -2, -1, "空文档原样发");
        plan(1, 0, false, -1, -1, TEXT, "", "", 1, 0, "空文档且选区未知时原样发");
        // getExtractedText 确认全文为空的真输入框（网页表单里还没填的标题框）：哪个方向都不发。
        planWith(-2, -1, false, 0, 0, TEXT, "", "", null, true, 0, 0, "确认为空的输入框不再左移上移");
        planWith(1, 1, false, 0, 0, TEXT, "", "", null, true, 0, 0, "确认为空的输入框不再右移下移");
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
        // 选择模式收缩：开头一侧按报来的活动端算。
        plan(-5, 0, true, 1, 3, PASSWORD, "x", "x", -3, 0, "密码框收缩到开头为止");
        plan(-5, 0, true, 3, 1, PASSWORD, "x", "x", -1, 0, "密码框向左延伸到开头为止");
        plan(-5, 0, true, 0, 3, PASSWORD, "x", "x", -3, 0, "密码框方向不确定时按右端截断");
        planWith(0, -1, false, 0, 2, PASSWORD, "x", "x", "x", true, 0, 0, "密码框不读另一侧，上键收起不发");
        planWith(-1, 0, false, 0, 0, PASSWORD, "", "", null, true, 0, 0, "密码框不问是否为空");
        check(reads == 0, "密码框一次也不读文字");
    }

    /** 编辑器的选区与回报一致：有选区时 getSelectedText 返回非空，全文不经 getExtractedText 确认。 */
    private static void plan(int horizontal, int vertical, boolean selecting, int start, int end, int inputType,
            String before, String after, int expectedHorizontal, int expectedVertical, String message) {
        String selected = start >= 0 && end >= 0 && start != end ? "x" : null;
        planWith(horizontal, vertical, selecting, start, end, inputType, before, after, selected, false,
            expectedHorizontal, expectedVertical, message);
    }

    private static void planWith(int horizontal, int vertical, boolean selecting, int start, int end, int inputType,
            String before, String after, String selected, boolean confirmsEmpty,
            int expectedHorizontal, int expectedVertical, String message) {
        TextCursorEdgePolicy.Plan plan = TextCursorEdgePolicy.plan(horizontal, vertical, selecting, start, end,
            inputType, new Reads(before, after, selected, confirmsEmpty));
        check(plan.horizontal() == expectedHorizontal && plan.vertical() == expectedVertical,
            message + "：得到 " + plan);
    }

    /** 编辑器按请求长度从靠近光标的一端截取，与 getTextBeforeCursor / getTextAfterCursor 一致；每次回读都计数。 */
    private static class Reads implements TextCursorEdgePolicy.Editor {
        private final String before;
        private final String after;
        private final String selected;
        private final boolean confirmsEmpty;

        Reads(String before, String after, String selected, boolean confirmsEmpty) {
            this.before = before;
            this.after = after;
            this.selected = selected;
            this.confirmsEmpty = confirmsEmpty;
        }

        @Override public CharSequence before(int limit) {
            reads++;
            return before == null || before.length() <= limit ? before : before.substring(before.length() - limit);
        }

        @Override public CharSequence after(int limit) {
            reads++;
            return after == null || after.length() <= limit ? after : after.substring(0, limit);
        }

        @Override public CharSequence selected() {
            reads++;
            return selected;
        }

        @Override public boolean confirmsEmpty() {
            reads++;
            return confirmsEmpty;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
