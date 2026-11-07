package app.msime.android;

import java.util.Map;

/**
 * 14 键与 18 键字母键右上角的符号（#11063）：一键一个符号，不放数字（数字由顶部数字行提供）。14 键是 `QW；ER# TY… UI( OP) / AS@ DF- GH、 JK： L" / ZX~ CV? BN! M/`，`% * & ¥` 留在「符」面板；18 键是 `Q； WE… RT- Y¥ U& IO( P) / A@ SD# FG% H* JK： L" / Z~ XC、 V? BN! M/`。全拼、双拼和五笔共用这两张表，与 iOS、HarmonyOS 是同一份。
 *
 * <p>表里存的是 ASCII 或本来就不是 ASCII 的字符本身。中文标点生效时，Engine 会转写的那几个（`( ) " ? ! : ;`）键面画中文形式（{@link #face}），输入交给 Engine 的标点路由（{@link #routesThroughEngine}），其余按字面上屏，与 26 键的 {@link LetterHintTable} 相同。
 */
public final class GridLetterHintTable {
    private static final Map<String, String> FOURTEEN = Map.ofEntries(
        Map.entry("qw", ";"), Map.entry("er", "#"), Map.entry("ty", "…"), Map.entry("ui", "("),
        Map.entry("op", ")"),
        Map.entry("as", "@"), Map.entry("df", "-"), Map.entry("gh", "、"), Map.entry("jk", ":"),
        Map.entry("l", "\""),
        Map.entry("zx", "~"), Map.entry("cv", "?"), Map.entry("bn", "!"), Map.entry("m", "/"));

    private static final Map<String, String> EIGHTEEN = Map.ofEntries(
        Map.entry("q", ";"), Map.entry("we", "…"), Map.entry("rt", "-"), Map.entry("y", "¥"),
        Map.entry("u", "&"), Map.entry("io", "("), Map.entry("p", ")"),
        Map.entry("a", "@"), Map.entry("sd", "#"), Map.entry("fg", "%"), Map.entry("h", "*"),
        Map.entry("jk", ":"), Map.entry("l", "\""),
        Map.entry("z", "~"), Map.entry("xc", "、"), Map.entry("v", "?"), Map.entry("bn", "!"),
        Map.entry("m", "/"));

    private GridLetterHintTable() {}

    /**
     * 一个字母键的符号。
     *
     * @param key 14 键或 18 键的键（{@link FourteenKeyLayout#rows}、{@link EighteenKeyLayout#rows}）
     * @param eighteen 查 18 键的表；否则查 14 键的表
     * @return 符号；不是这种键盘上的键时返回 null
     */
    public static String hint(FourteenKeyLayout.Key key, boolean eighteen) {
        if (key == null) return null;
        return (eighteen ? EIGHTEEN : FOURTEEN).get(key.letters());
    }

    /**
     * 右上角画的键面，也是滑动时气泡显示的字符。
     *
     * @param chinesePunctuation 中文标点是否生效（{@code MSIMEInputService.sendsChinesePunctuation()}）
     * @param marks 引号与括号样式；直角引号下 `"` 画『
     * @return 中文标点生效时 `( ) " ? ! : ;` 换成中文形式，其余照旧；没有符号时返回 null
     */
    public static String face(FourteenKeyLayout.Key key, boolean eighteen, boolean chinesePunctuation,
            ChineseSymbolFaces.Marks marks) {
        String hint = hint(key, eighteen);
        if (hint == null || !chinesePunctuation) return hint;
        return ChineseSymbolFaces.face(hint, true, marks);
    }

    /** 中文标点生效时这个符号是否交给 Engine 的标点路由上屏；规则与 26 键相同，见 {@link LetterHintTable#routesThroughEngine}。 */
    public static boolean routesThroughEngine(String hint) {
        return LetterHintTable.routesThroughEngine(hint);
    }
}
