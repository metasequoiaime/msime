package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * 拼音九键与笔画键盘左侧符号栏的内容：字母键面与数字键面各自的默认符号，以及用户在设置里自定义的符号表（`platform.android.nine_key_symbols` / `platform.android.nine_key_digit_symbols`）的解析与校验。
 *
 * <p>符号栏一屏显示 {@link #VISIBLE_ROWS} 个，多出来的上下滚动；原来三个符号铺满三行键高，间距大到还能再放两个（#5574）。拼音九键的数字键面是用来打数字和算式的，左栏换成四则运算符号，叹号挪到运算符号后面，滚动才看得到（#5590）。存储格式是用空格分开的符号，读不出或不合规时整张表回到默认值。
 */
public final class NineKeySidebarPolicy {
    /** 符号栏一屏最多显示几个符号；符号不足这么多时按实际个数均分整栏高度。 */
    public static final int VISIBLE_ROWS = 5;
    /** 每个符号键至少多高（dp）：键盘高度调到最矮时一屏少放几个，不把键压得比字还矮。 */
    public static final int MIN_ROW_HEIGHT_DP = 28;
    /** 一张符号表最多几个符号。 */
    public static final int MAX_SYMBOLS = 30;
    /** 一个符号最多几个码位（「……」「:-)」这类短串也允许）。 */
    public static final int MAX_SYMBOL_CODE_POINTS = 4;
    /** 存储文本的最大长度（UTF-16 单元）。 */
    public static final int MAX_TEXT_LENGTH = 200;

    /** 字母键面的默认符号：前三个仍是原来的 ，。？，！ 仍在右列最下面，所以这里不重复。 */
    public static final List<String> DEFAULT_LETTER_SYMBOLS =
        List.of("，", "。", "？", "、", "：", "；", "……", "～", "@");
    /** 数字键面的默认符号：先是四则运算和算式里常用的半角符号，中文标点（含原来在右列的 ！）排在后面。 */
    public static final List<String> DEFAULT_DIGIT_SYMBOLS =
        List.of("+", "-", "*", "/", "=", "%", "(", ")", ":", "@", "，", "。", "？", "！");

    private NineKeySidebarPolicy() {}

    /**
     * 把用户输入或存储里的文本解析成符号表：按空白（含全角空格）分开，去掉重复项，保留第一次出现的顺序。
     *
     * @return 符号表；文本为空、有符号超过 {@link #MAX_SYMBOL_CODE_POINTS} 个码位或含控制字符、符号超过 {@link #MAX_SYMBOLS} 个、文本超过 {@link #MAX_TEXT_LENGTH} 时返回 null
     */
    public static List<String> parse(String text) {
        if (text == null || text.length() > MAX_TEXT_LENGTH) return null;
        Set<String> symbols = new LinkedHashSet<>();
        StringBuilder token = new StringBuilder();
        int index = 0;
        while (index <= text.length()) {
            int codePoint = index < text.length() ? text.codePointAt(index) : ' ';
            if (Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint)) {
                if (token.length() > 0) {
                    symbols.add(token.toString());
                    token.setLength(0);
                }
            } else {
                if (Character.isISOControl(codePoint)) return null;
                token.appendCodePoint(codePoint);
                if (token.codePointCount(0, token.length()) > MAX_SYMBOL_CODE_POINTS) return null;
            }
            index += index < text.length() ? Character.charCount(codePoint) : 1;
        }
        if (symbols.isEmpty() || symbols.size() > MAX_SYMBOLS) return null;
        return List.copyOf(symbols);
    }

    /** 存储用的规范文本：解析后的符号以一个空格连接；不合规时为 null。 */
    public static String normalize(String text) {
        List<String> symbols = parse(text);
        return symbols == null ? null : format(symbols);
    }

    /** 符号表的存储文本：符号以一个空格连接。 */
    public static String format(List<String> symbols) {
        return String.join(" ", symbols);
    }

    /** 实际显示的符号：存储的文本合规时用它，否则用 `fallback`。 */
    public static List<String> symbols(String stored, List<String> fallback) {
        List<String> parsed = parse(stored);
        return parsed == null ? fallback : parsed;
    }

    /** 字母键面（拼音九键、笔画）显示的符号。 */
    public static List<String> letterSymbols(String stored) {
        return symbols(stored, DEFAULT_LETTER_SYMBOLS);
    }

    /** 数字键面显示的符号。 */
    public static List<String> digitSymbols(String stored) {
        return symbols(stored, DEFAULT_DIGIT_SYMBOLS);
    }

    /**
     * 当前键面的左栏符号：拼音九键的数字键面用数字那张表（#5590），字母键面和笔画键盘用字母那张。
     *
     * @param digits 是不是拼音九键的数字键面
     * @param letterStored 本地设置里字母键面的符号表文本
     * @param digitStored 本地设置里数字键面的符号表文本
     */
    public static List<String> sidebarSymbols(boolean digits, String letterStored, String digitStored) {
        return digits ? digitSymbols(digitStored) : letterSymbols(letterStored);
    }

    /**
     * 符号栏里每个符号占的高度（像素）：一屏放 {@link #VISIBLE_ROWS} 个，符号少于这么多时均分整栏，不留空白；放这么多会让每个低于 `minRowHeight` 时少放几个。
     *
     * @param railHeight 符号栏的可见高度
     * @param count 符号个数
     * @param minRowHeight 每个符号的最小高度（像素，{@link #MIN_ROW_HEIGHT_DP} 换算后）
     */
    public static int rowHeight(int railHeight, int count, int minRowHeight) {
        if (railHeight <= 0) return 0;
        int rows = Math.min(count, VISIBLE_ROWS);
        if (minRowHeight > 0) rows = Math.min(rows, railHeight / minRowHeight);
        return railHeight / Math.max(1, rows);
    }

    /** 列出时给设置页显示的摘要：最多前 `limit` 个符号，后面用「等 N 个」收尾。 */
    public static String summary(List<String> symbols, int limit) {
        if (symbols.size() <= limit) return String.join(" ", symbols);
        List<String> head = new ArrayList<>(symbols.subList(0, Math.max(0, limit)));
        return String.join(" ", head) + " 等 " + symbols.size() + " 个";
    }
}
