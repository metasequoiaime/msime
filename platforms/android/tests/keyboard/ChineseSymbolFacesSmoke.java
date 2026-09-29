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
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 0, "emoji", true),
            "local utility mode uses ASCII faces");
        // The user's own switch gates the faces too: a key that shows 。 and commits . is worse
        // than either choice on its own.
        check(!ChineseSymbolFaces.shouldUseChineseFaces(false, 0, "none", false),
            "turning Chinese punctuation off returns the ASCII faces");
        check("，".equals(ChineseSymbolFaces.face(",", true))
                && ",".equals(ChineseSymbolFaces.face(",", false)),
            "the face follows the mode it is given");
        System.out.println("Android Chinese symbol faces: punctuation and language switching passed");
    }
}
