import app.msime.android.keyboard.EnglishSuggestionPolicy;

public final class EnglishSuggestionPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(EnglishSuggestionPolicy.currentWord("我用iph").equals("iph"), "suffix word");
        check(EnglishSuggestionPolicy.currentWord("hello wor").equals("wor"), "word boundary");
        check(EnglishSuggestionPolicy.currentWord("中文ｉｐｈ").equals("iph"),
            "full-width suffix word should normalize");
        check(EnglishSuggestionPolicy.currentWord("中文Ｈｅ").equals("He"),
            "full-width capitalization should normalize");
        check(EnglishSuggestionPolicy.currentWord("done. ").isEmpty(), "punctuation boundary");
        check(EnglishSuggestionPolicy.currentWord("don't").equals("don't"), "apostrophe stays in the current word");
        var replacement = EnglishSuggestionPolicy.replacement("iph", "iphone", false);
        check(replacement != null && replacement.deleteCount() == 3
            && replacement.insert().equals("iphone"), "replacement span");
        var capitalized = EnglishSuggestionPolicy.replacement("Hel", "hello", true);
        check(capitalized != null && capitalized.insert().equals("Hello"), "capitalized replacement");
        var fullWidthCapitalized = EnglishSuggestionPolicy.replacement("He", "hello", true);
        check(fullWidthCapitalized != null && fullWidthCapitalized.deleteCount() == 2
            && fullWidthCapitalized.insert().equals("Hello"),
            "normalized full-width replacement keeps initial capitalization and length");
        check(EnglishSuggestionPolicy.replacement("hello", "hello", false) == null,
            "identical replacement is omitted");
        var segmented = EnglishSuggestionPolicy.replacement("imchinese", "I'm Chinese", false);
        check(segmented != null && segmented.deleteCount() == 9
            && segmented.insert().equals("I'm Chinese"), "split correction replaces the entire current word");
        System.out.println("Android English suggestion policy: boundaries and replacement passed");
    }
}
