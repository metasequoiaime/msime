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
        var ascii = QuickPunctuationPolicy.entries(true, 0, "none");
        check(ascii.get(0).face().equals(",") && ascii.get(0).input() == ',',
            "English mode uses ASCII faces");
        check(QuickPunctuationPolicy.entries(false, 0, "emoji").get(0).face().equals(","),
            "Local mode uses ASCII faces");
        System.out.println("Android quick punctuation: Chinese, Japanese and ASCII modes passed");
    }
}
