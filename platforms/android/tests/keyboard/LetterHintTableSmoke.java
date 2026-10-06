import app.msime.android.LetterHintTable;

public final class LetterHintTableSmoke {
    public static void main(String[] args) {
        String rows = "q1w2e3r4t5y6u7i8o9p0a@s#d¥f%g&h*j(k)l\"z~x…c、v?b!n-m/";
        int count = 0;
        for (int index = 0; index < rows.length(); index += 2) {
            String letter = rows.substring(index, index + 1);
            String hint = rows.substring(index + 1, index + 2);
            check(hint.equals(LetterHintTable.hint(letter)), "hint of " + letter);
            check(hint.equals(LetterHintTable.hint(letter.toUpperCase(java.util.Locale.ROOT))),
                "uppercase " + letter + " reads the same hint");
            count++;
        }
        check(count == 26, "all 26 letters carry a hint");
        check(LetterHintTable.hint("1") == null && LetterHintTable.hint("") == null
            && LetterHintTable.hint(null) == null && LetterHintTable.hint("ab") == null,
            "non-letters have no hint");
        check(!LetterHintTable.hasHint("，") && LetterHintTable.hasHint("m"), "hasHint");
        System.out.println("Android letter hints: q1…p0 / a@…l\" / z~…m/ passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
