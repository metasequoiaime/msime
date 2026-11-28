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
        // 契约第 7 版：View 报告的保留英文的键画 ASCII 面，其余键照旧；日文键面不看这个集合。
        ChineseSymbolFaces.Marks preserved = ChineseSymbolFaces.Marks.of("corner", "solid", true, false, "`\\'");
        check(ChineseSymbolFaces.face("`", true, preserved).equals("`")
                && ChineseSymbolFaces.face("\\", true, preserved).equals("\\")
                && ChineseSymbolFaces.face("'", true, preserved).equals("'"), "preserved keys keep their ASCII faces");
        check(ChineseSymbolFaces.face("\"", true, preserved).equals("『")
                && ChineseSymbolFaces.face(",", true, preserved).equals("，"), "other keys keep their Chinese faces");
        check(ChineseSymbolFaces.japaneseFace("\\").equals("、"), "Japanese faces ignore the preserved set");
        check(!new ChineseSymbolFaces.Marks(false, false, true, false, null).preservesEnglish("`"),
            "a missing set preserves nothing");
        // 日语方案的键面与引擎 policy.json 的 japanese 表逐字一致：键面画什么就上屏什么。
        check(ChineseSymbolFaces.japaneseFace(",").equals("、")
                && ChineseSymbolFaces.japaneseFace("\\").equals("、"), "Japanese comma and ideographic comma");
        check(ChineseSymbolFaces.japaneseFace(".").equals("。"), "Japanese period");
        check(ChineseSymbolFaces.japaneseFace("[").equals("「")
                && ChineseSymbolFaces.japaneseFace("]").equals("」"), "Japanese square brackets");
        check(ChineseSymbolFaces.japaneseFace("{").equals("『")
                && ChineseSymbolFaces.japaneseFace("}").equals("』"), "Japanese double corner quotes");
        check(ChineseSymbolFaces.japaneseFace("/").equals("・")
                && ChineseSymbolFaces.japaneseFace("~").equals("〜"), "Japanese middle dot and long vowel");
        check(ChineseSymbolFaces.japaneseFace("?").equals("？")
                && ChineseSymbolFaces.japaneseFace("!").equals("！"), "Japanese full-width question and exclamation");
        check(ChineseSymbolFaces.shouldUseJapaneseFaces(false, 3, "none", true),
            "Japanese scheme with the Chinese punctuation switch on draws Japanese faces");
        check(!ChineseSymbolFaces.shouldUseJapaneseFaces(false, 3, "none", false),
            "Japanese scheme with the switch off draws ASCII faces, as the Engine writes ASCII");
        check(!ChineseSymbolFaces.shouldUseJapaneseFaces(true, 3, "none", true)
                && !ChineseSymbolFaces.shouldUseJapaneseFaces(false, 3, "calculator", true)
                && !ChineseSymbolFaces.shouldUseJapaneseFaces(false, 0, "none", true),
            "English, local tool and non-Japanese schemes never draw Japanese faces");
        check(ChineseSymbolFaces.japaneseFace("@").equals("@"), "unmapped symbol keeps its face in Japanese");
        check(ChineseSymbolFaces.japaneseFace(null).isEmpty(), "null face is bounded in Japanese");
        System.out.println("Android Chinese symbol faces: punctuation and language switching passed");
    }
}
