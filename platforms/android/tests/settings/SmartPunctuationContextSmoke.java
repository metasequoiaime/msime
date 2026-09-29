import app.msime.android.SmartPunctuationContext;

public final class SmartPunctuationContextSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        for (char value : new char[] {',', '.', ':', '?', '!', '@', '[', '\\'})
            check(SmartPunctuationContext.isAsciiPunctuation(value), "punctuation classification");
        for (char value : new char[] {'0', '9', 'a', 'Z'})
            check(!SmartPunctuationContext.isAsciiPunctuation(value), "alphanumeric classification");
        check(SmartPunctuationContext.precedingCodePoint(null) == 0, "null context");
        check(SmartPunctuationContext.precedingCodePoint("") == 0, "empty context");
        check(SmartPunctuationContext.precedingCodePoint("fixture7") == '7', "digit context");
        check(SmartPunctuationContext.precedingCodePoint("fixtureZ") == 'Z', "letter context");
        check(SmartPunctuationContext.precedingCodePoint("fixture中") == '中', "CJK context");
        check(SmartPunctuationContext.precedingCodePoint("fixture🌲") == 0x1f332,
            "supplementary scalar context");
        check(SmartPunctuationContext.precedingCodePoint("fixture\ud83c") == 0,
            "isolated high surrogate context");
        check(SmartPunctuationContext.precedingCodePoint("fixture\udf32") == 0,
            "isolated low surrogate context");
        System.out.println("Android smart punctuation context: bounded preceding scalar passed");
    }
}
