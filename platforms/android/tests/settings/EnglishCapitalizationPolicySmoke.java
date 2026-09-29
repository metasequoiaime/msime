import app.msime.android.EnglishCapitalizationPolicy;

public final class EnglishCapitalizationPolicySmoke {
    static void expect(boolean expected, EnglishCapitalizationPolicy.Mode mode, String context) {
        if (EnglishCapitalizationPolicy.shouldShift(mode, context) != expected) {
            throw new AssertionError(mode + " capitalization mismatch");
        }
    }

    public static void main(String[] args) {
        expect(false, EnglishCapitalizationPolicy.Mode.NONE, "");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, null);
        expect(true, EnglishCapitalizationPolicy.Mode.ALL_CHARACTERS, null);

        expect(true, EnglishCapitalizationPolicy.Mode.WORDS, "");
        expect(true, EnglishCapitalizationPolicy.Mode.WORDS, "hello ");
        expect(false, EnglishCapitalizationPolicy.Mode.WORDS, "hel");
        expect(false, EnglishCapitalizationPolicy.Mode.WORDS, "don't");
        expect(false, EnglishCapitalizationPolicy.Mode.WORDS, "don'");

        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "Hello");
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "Hello. ");
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "Really!\n");
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "Done.\u201d ");
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "Done.\u00a0");
        System.out.println("Android English capitalization policy passed");
    }
}
