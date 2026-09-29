import app.msime.android.CandidateTextPolicy;
import app.msime.android.WordCharacterPolicy.Edge;

/** The host's half of 以词定字: what to commit when the Engine declines the candidate. */
public final class CandidateTextPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check("你".equals(CandidateTextPolicy.hanCharacter("你好", Edge.FIRST)),
            "first takes the leading Han character");
        check("好".equals(CandidateTextPolicy.hanCharacter("你好", Edge.LAST)),
            "last takes the trailing Han character");
        // Three characters, because taking the second one instead of the last would still pass a
        // two-character case. This is the shape the shared Engine example pins as well.
        check("团".equals(CandidateTextPolicy.hanCharacter("代表团", Edge.LAST)),
            "last is the last Han character, not the second");
        check("代".equals(CandidateTextPolicy.hanCharacter("代表团", Edge.FIRST)),
            "first is unaffected by the length");

        // "Last Han character" is not "last character": a trailing mark must not hide the word.
        check("好".equals(CandidateTextPolicy.hanCharacter("你好！", Edge.LAST)),
            "a trailing non-Han mark is skipped");
        check("表".equals(CandidateTextPolicy.hanCharacter("（代表）", Edge.LAST)),
            "surrounding marks are skipped at both ends");
        check("代".equals(CandidateTextPolicy.hanCharacter("（代表）", Edge.FIRST)),
            "a leading non-Han mark is skipped");

        check("〇".equals(CandidateTextPolicy.hanCharacter("〇", Edge.FIRST)),
            "〇 counts as Han");
        // A supplementary-plane ideograph is one character, not two surrogate halves.
        String extensionB = new String(Character.toChars(0x20000));
        check(extensionB.equals(CandidateTextPolicy.hanCharacter(extensionB + "字", Edge.FIRST)),
            "a surrogate pair is returned whole");
        check("字".equals(CandidateTextPolicy.hanCharacter(extensionB + "字", Edge.LAST)),
            "scanning past a surrogate pair stays aligned");

        check(CandidateTextPolicy.hanCharacter("hello", Edge.FIRST) == null,
            "a candidate with no Han character yields none");
        check(CandidateTextPolicy.hanCharacter("", Edge.FIRST) == null
                && CandidateTextPolicy.hanCharacter(null, Edge.FIRST) == null
                && CandidateTextPolicy.hanCharacter("你好", Edge.NONE) == null,
            "empty, null and NONE yield none rather than throwing");

        // The source commits the whole candidate when its own extraction comes back empty, rather
        // than swallowing the key press and leaving nothing on screen.
        check("hello".equals(CandidateTextPolicy.fallbackCommit("hello", Edge.FIRST)),
            "a candidate with no Han character commits whole");
        check("你".equals(CandidateTextPolicy.fallbackCommit("你好", Edge.FIRST)),
            "a candidate with Han text still yields just the character");
        check(CandidateTextPolicy.fallbackCommit("", Edge.FIRST) == null
                && CandidateTextPolicy.fallbackCommit(null, Edge.FIRST) == null,
            "there is nothing to commit for an empty candidate");
        System.out.println("Android candidate text policy passed");
    }
}
