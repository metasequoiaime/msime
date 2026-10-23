import app.msime.android.TextTrackpadText;
import java.util.Locale;

/** 合成文字的有界预览和绝对选区计算。 */
public final class TextTrackpadTextSmoke {
    public static void main(String[] args) {
        wordSelection();
        lineSelection();
        boundedPreviews();
        editorReads();
        documentEnd();
        System.out.println("Android text trackpad text: bounded previews, words, lines and passwords passed");
    }

    private static void documentEnd() {
        check(TextTrackpadText.endsDocument("alpha beta", 8, "ta"), "完整文本中间光标仍可确认末尾");
        check(!TextTrackpadText.endsDocument("alpha beta", 8, "ta hidden"), "提取片段不能冒充文档末尾");
        check(!TextTrackpadText.endsDocument("alpha beta", 8, "xx"), "回读内容变化时拒绝旧片段");
        check(!TextTrackpadText.endsDocument("alpha beta", 8, null), "回读拒绝时保留未知末尾");
        range(TextTrackpadText.word("alpha beta", 0, 8, true,
            TextTrackpadText.endsDocument("alpha beta", 8, "ta"), Locale.ENGLISH), 6, 10);
        range(TextTrackpadText.line("alpha\nbeta", 0, 8, true,
            TextTrackpadText.endsDocument("alpha\nbeta", 8, "ta")), 6, 10);
    }

    private static void wordSelection() {
        range(TextTrackpadText.word("alpha beta gamma", 0, 8, true, true, Locale.ENGLISH), 6, 10);
        range(TextTrackpadText.word("alpha beta gamma", 500, 508, false, false, Locale.ENGLISH), 506, 510);
        range(TextTrackpadText.word("alpha beta gamma", 0, 0, true, true, Locale.ENGLISH), 0, 5);
        range(TextTrackpadText.word("alpha beta gamma", 0, 16, true, true, Locale.ENGLISH), 11, 16);
        range(TextTrackpadText.word("x 😀 y", 30, 33, false, false, Locale.ENGLISH), 32, 34);
        check(TextTrackpadText.word("partial", 8, 11, false, false, Locale.ENGLISH) == null,
            "两端截断的词不能误当完整词");
        check(TextTrackpadText.word("alpha beta", 10, 12, false, true, Locale.ENGLISH) == null,
            "左边界未知时不能选片段开头");
        check(TextTrackpadText.word("alpha beta", 10, 18, true, false, Locale.ENGLISH) == null,
            "右边界未知时不能选片段结尾");
        check(TextTrackpadText.word(" ", 0, 0, true, true, Locale.ENGLISH) == null,
            "空白不作为词");
        check(TextTrackpadText.word("alpha", Integer.MAX_VALUE, Integer.MAX_VALUE, true, true,
            Locale.ENGLISH) == null, "绝对偏移不溢出");
    }

    private static void lineSelection() {
        range(TextTrackpadText.line("one\r\ntwo\nthree", 100, 106, false, false), 105, 108);
        range(TextTrackpadText.line("one\ntwo", 0, 1, true, false), 0, 3);
        range(TextTrackpadText.line("one\ntwo", 0, 6, true, true), 4, 7);
        check(TextTrackpadText.line("one\ntwo", 20, 21, false, true) == null,
            "首行截断时不选半行");
        check(TextTrackpadText.line("one\ntwo", 20, 26, true, false) == null,
            "末行截断时不选半行");
        check(TextTrackpadText.line("abc", -1, 0, true, true) == null, "拒绝负偏移");
        check(TextTrackpadText.line("abc", 0, 4, true, true) == null, "拒绝片段外的光标");
        range(TextTrackpadText.line("a\r\nb", 0, 2, true, true), 0, 1);
    }

    private static void boundedPreviews() {
        var value = TextTrackpadText.preview("prefix", "suffix", 0, false);
        check(value.text().equals("prefixsuffix") && value.selectionStart() == 6
            && value.selectionEnd() == 6 && !value.masked(), "折叠光标预览");
        value = TextTrackpadText.preview("prefix", "suffix", 1000000, false);
        check(value.text().equals("prefix…suffix") && value.selectionStart() == 6
            && value.selectionEnd() == 7, "长选区只显示占位，不读取整段");
        value = value.direction(true);
        check(value.selectionStart() == 7 && value.selectionEnd() == 6, "反向选区光标显示在左端");
        CharSequence forbidden = new CharSequence() {
            @Override public int length() { throw new AssertionError("密码不许读取上下文"); }
            @Override public char charAt(int at) { throw new AssertionError("密码不许读取上下文"); }
            @Override public CharSequence subSequence(int start, int end) { throw new AssertionError("密码不许读取上下文"); }
            @Override public String toString() { throw new AssertionError("密码不许读取上下文"); }
        };
        value = TextTrackpadText.preview(forbidden, forbidden, 1000000, true);
        check(value.masked() && value.text().equals("••••••••"), "密码固定掩码");
        check(TextTrackpadText.preview(null, "x", 0, false) == null, "读取被拒绝时无预览");
        value = TextTrackpadText.preview("x".repeat(95) + "😀" + "x".repeat(95),
            "x".repeat(95) + "😀", 0, false);
        check(value.text().length() <= 192 && wellFormed(value.text()), "有界截取不拆代理对");
        value = TextTrackpadText.preview("line1\nline2\t", "\u0000end", 0, false);
        check(!value.text().contains("\n") && !value.text().contains("\u0000"), "预览不保留布局控制字符");
    }

    private static void editorReads() {
        java.util.function.IntFunction<CharSequence> forbidden = limit -> {
            throw new AssertionError("密码或未知选区不能跨进程读取");
        };
        var value = TextTrackpadText.readPreview(forbidden, forbidden, -1, true);
        check(value != null && value.masked(), "密码回读入口直接给固定掩码");
        check(TextTrackpadText.readPreview(forbidden, forbidden, -1, false) == null,
            "未知选区先等待真实回报");
        int[] reads = new int[2];
        value = TextTrackpadText.readPreview(limit -> {
            check(limit <= 96, "光标前只读取有界上下文"); reads[0]++; return "before";
        }, limit -> {
            check(limit <= 96, "光标后只读取有界上下文"); reads[1]++; return "after";
        }, 1000000, false);
        check(value != null && value.text().equals("before…after") && reads[0] == 1 && reads[1] == 1,
            "长选区不读取选中文字，只取两侧有界文字");
        check(TextTrackpadText.readPreview(limit -> { throw new IllegalStateException("synthetic"); },
            limit -> "", 0, false) == null, "断开的编辑器没有预览");
        check(TextTrackpadText.readPreview(limit -> null, limit -> "", 0, false) == null,
            "拒绝回读的编辑器没有预览");
    }

    private static boolean wellFormed(String text) {
        for (int at = 0; at < text.length(); at++) {
            char c = text.charAt(at);
            if (Character.isHighSurrogate(c)) {
                if (++at >= text.length() || !Character.isLowSurrogate(text.charAt(at))) return false;
            } else if (Character.isLowSurrogate(c)) return false;
        }
        return true;
    }

    private static void range(TextTrackpadText.Range value, int start, int end) {
        check(value != null && value.start() == start && value.end() == end,
            "绝对选区应为 " + start + ", " + end + "，实际 " + value);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
