import app.msime.android.JapaneseRomajiLayout;
import java.util.List;

public final class JapaneseRomajiLayoutSmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("日语罗马字长音键布局不符合预期");
    }

    public static void main(String[] args) {
        List<List<String>> original = List.of(
            List.of("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
            List.of("a", "s", "d", "f", "g", "h", "j", "k", "l"),
            List.of("z", "x", "c", "v", "b", "n", "m"));
        var japanese = JapaneseRomajiLayout.rows(original, true);
        check(japanese.size() == 3);
        check(japanese.get(0).equals(original.get(0)));
        check(japanese.get(1).equals(List.of("a", "s", "d", "f", "g", "h", "j", "k", "l", "-")));
        check(japanese.get(2).equals(original.get(2)));
        check(original.get(1).size() == 9);
        check(original.get(2).size() == 7);
        check(JapaneseRomajiLayout.rows(original, false) == original);
        check(JapaneseRomajiLayout.active(true, false, "none"));
        check(!JapaneseRomajiLayout.active(true, true, "none"));
        check(!JapaneseRomajiLayout.active(false, false, "none"));
        check(!JapaneseRomajiLayout.active(true, false, "english"));
        check(JapaneseRomajiLayout.LONG_VOWEL_FACE.equals("ー"));
        check(JapaneseRomajiLayout.LONG_VOWEL_INPUT.equals("-"));
    }
}
