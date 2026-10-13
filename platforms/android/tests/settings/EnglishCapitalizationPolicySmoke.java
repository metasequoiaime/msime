import app.msime.android.EnglishCapitalizationPolicy;

public final class EnglishCapitalizationPolicySmoke {
    static void expect(boolean expected, EnglishCapitalizationPolicy.Mode mode, String context) {
        if (EnglishCapitalizationPolicy.shouldShift(mode, context) != expected) {
            throw new AssertionError(mode + " capitalization mismatch");
        }
    }

    public static void main(String[] args) {
        // 关闭时不受编辑器模式或句首上下文影响；开启后沿用已有规则。
        for (EnglishCapitalizationPolicy.Mode mode : EnglishCapitalizationPolicy.Mode.values()) {
            for (String context : new String[] {null, "", "\n", "Hello. ", "hello "}) {
                if (EnglishCapitalizationPolicy.shouldShift(false, mode, context)) {
                    throw new AssertionError("disabled capitalization must stay lowercase");
                }
                if (EnglishCapitalizationPolicy.shouldShift(true, mode, context)
                        != EnglishCapitalizationPolicy.shouldShift(mode, context)) {
                    throw new AssertionError("enabled capitalization must preserve editor rules");
                }
            }
        }
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
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "   ");
        expect(true, EnglishCapitalizationPolicy.Mode.SENTENCES, "\u4f60\u597d\u3002 ");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "Hello.");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "Really?");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "Done.\u201d");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "3.");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "\u4f60\u597d\u3002");
        expect(false, EnglishCapitalizationPolicy.Mode.SENTENCES, "Hi) ");
        System.out.println("Android English capitalization policy passed");
    }
}
