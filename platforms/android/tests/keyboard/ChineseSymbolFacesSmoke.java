import app.msime.android.ChineseSymbolFaces;

public final class ChineseSymbolFacesSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ChineseSymbolFaces.face(",", true).equals("，"), "comma face follows Chinese punctuation");
        check(ChineseSymbolFaces.face(".", true).equals("。"), "period face follows Chinese punctuation");
        check(ChineseSymbolFaces.face("\\", true).equals("、"), "backslash explains ideographic comma");
        check(ChineseSymbolFaces.face("[", true).equals("【"), "bracket face follows Chinese punctuation");
        check(ChineseSymbolFaces.face("_", true).equals("——"), "underscore face follows Chinese punctuation");
        check(ChineseSymbolFaces.face("@", true).equals("@"), "unmapped symbol keeps its inserted face");
        check(ChineseSymbolFaces.face("\\", false).equals("\\"), "English mode keeps ASCII face");
        check(ChineseSymbolFaces.face(null, true).isEmpty(), "null face is bounded to empty text");
        check(ChineseSymbolFaces.shouldUseChineseFaces(false, 0, "none", true),
            "ordinary Chinese scheme uses Chinese faces");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(true, 0, "none", true),
            "dedicated English mode uses ASCII faces");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 3, "none", true),
            "Japanese scheme uses ASCII faces");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 4, "none", true),
            "Korean scheme uses half-width ASCII faces whatever the Chinese punctuation switch says");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 7, "none", true),
            "Vietnamese scheme uses half-width ASCII faces whatever the Chinese punctuation switch says");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 8, "none", true),
            "Tibetan scheme uses half-width ASCII faces whatever the Chinese punctuation switch says");
        check(ChineseSymbolFaces.shouldUseChineseFaces(false, 5, "none", true)
                && ChineseSymbolFaces.shouldUseChineseFaces(false, 6, "none", true)
                && ChineseSymbolFaces.shouldUseChineseFaces(false, 9, "none", true),
            "Cantonese, Zhuyin and Stroke use Chinese faces");
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 0, "emoji", true),
            "local utility mode uses ASCII faces");
        // The user's own switch gates the faces too: a key that shows 。 and commits . is worse
        // than either choice on its own.
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 0, "none", false),
            "turning Chinese punctuation off returns the ASCII faces");
        check("，".equals(ChineseSymbolFaces.face(",", true))
                && ",".equals(ChineseSymbolFaces.face(",", false)),
            "the face follows the mode it is given");
        // 引号与方头括号样式（View 的 punctuation_marks）只换 ' " [ ] 的键面。
        ChineseSymbolFaces.Marks styled = ChineseSymbolFaces.Marks.of("corner", "hollow");
        check(styled.cornerQuotes() && styled.hollowBrackets(), "View values parse");
        check(ChineseSymbolFaces.Marks.of("curly", "unknown").equals(ChineseSymbolFaces.Marks.DEFAULT),
            "unknown values fall back to the defaults");
        check(ChineseSymbolFaces.face("'", true, styled).equals("「"), "corner ' face");
        check(ChineseSymbolFaces.face("\"", true, styled).equals("『"), "corner \" face");
        check(ChineseSymbolFaces.face("[", true, styled).equals("〖"), "hollow [ face");
        check(ChineseSymbolFaces.face("]", true, styled).equals("〗"), "hollow ] face");
        check(ChineseSymbolFaces.face(",", true, styled).equals("，"), "other faces are untouched");
        check(ChineseSymbolFaces.face("'", false, styled).equals("'"), "English mode keeps ASCII face");
        check(ChineseSymbolFaces.face("'", true, ChineseSymbolFaces.Marks.DEFAULT).equals("‘")
                && ChineseSymbolFaces.face("[", true, null).equals("【"),
            "the default style keeps the contract faces");
        // 顿号键面随 View 的顿号开关：`\` 关着画 `\`，`/` 开着画 `、`。
        ChineseSymbolFaces.Marks slashOnly = ChineseSymbolFaces.Marks.of("curly", "solid", false, true);
        check(ChineseSymbolFaces.face("\\", true, slashOnly).equals("\\"),
            "a disabled backslash key keeps its ASCII face");
        check(ChineseSymbolFaces.face("/", true, slashOnly).equals("、"),
            "an enabled slash key shows the ideographic comma");
        check(ChineseSymbolFaces.face("/", true, ChineseSymbolFaces.Marks.DEFAULT).equals("/"),
            "slash keeps its ASCII face while disabled by default");
        check(ChineseSymbolFaces.face("\\", true, ChineseSymbolFaces.Marks.DEFAULT).equals("、"),
            "the default marks keep backslash as the ideographic comma key");
        System.out.println("Android Chinese symbol faces: punctuation and language switching passed");
    }
}
