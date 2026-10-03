import app.msime.android.QuickPunctuationPolicy;

public final class QuickPunctuationPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        var chinese = QuickPunctuationPolicy.entries(false, 0, "none");
        check(chinese.size() == 7 && chinese.get(0).face().equals("，")
            && chinese.get(0).input() == ',', "Chinese quick punctuation uses full-width faces");
        check(chinese.get(4).face().equals("、") && chinese.get(4).input() == '\\',
            "Chinese ideographic comma maps to backslash");
        var japanese = QuickPunctuationPolicy.entries(false, 3, "none");
        check(japanese.get(0).face().equals("、") && japanese.get(4).input() == '[',
            "Japanese quick punctuation uses Japanese faces and ASCII engine inputs");
        var korean = QuickPunctuationPolicy.entries(false, 4, "none");
        check(korean.get(0).face().equals(",") && korean.get(1).face().equals("."),
            "Korean quick punctuation uses half-width ASCII faces");
        var vietnamese = QuickPunctuationPolicy.entries(false, 7, "none");
        check(vietnamese.equals(korean), "Vietnamese quick punctuation is half-width ASCII, as Korean's is");
        check(QuickPunctuationPolicy.entries(false, 5, "none").equals(chinese),
            "Cantonese quick punctuation is the Chinese set");
        check(QuickPunctuationPolicy.entries(false, 8, "none").equals(chinese),
            "Stroke quick punctuation is the Chinese set; its wildcard is the letter x, not a mark");
        var zhuyin = QuickPunctuationPolicy.entries(false, 6, "none");
        check(zhuyin.size() == 7 && zhuyin.get(0).face().equals("，") && zhuyin.get(0).input() == '<'
            && zhuyin.get(1).face().equals("。") && zhuyin.get(1).input() == '>',
            "Zhuyin sends the Shift marks, since Dachen reads , and . as bopomofo");
        for (var entry : zhuyin) {
            check(",./;-0123456789".indexOf(entry.input()) < 0, "no Zhuyin quick mark sends a Dachen key");
        }
        var ascii = QuickPunctuationPolicy.entries(true, 0, "none");
        check(ascii.get(0).face().equals(",") && ascii.get(0).input() == ',',
            "English mode uses ASCII faces");
        check(QuickPunctuationPolicy.entries(false, 0, "emoji").get(0).face().equals(","),
            "Local mode uses ASCII faces");
        System.out.println("Android quick punctuation: Chinese, Japanese, Korean, Zhuyin, Vietnamese and ASCII modes passed");
    }
}
