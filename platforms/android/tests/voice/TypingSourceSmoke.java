import app.msime.android.KeyboardScheme;
import app.msime.android.TypingSource;

public final class TypingSourceSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(TypingSource.resolve(KeyboardScheme.QUANPIN, false, "none") == TypingSource.QUANPIN);
        check(TypingSource.resolve(KeyboardScheme.QUANPIN_NINE_KEY, false, "") == TypingSource.NINE_KEY);
        check(TypingSource.resolve(KeyboardScheme.XIAOHE, false, null) == TypingSource.SHUANGPIN);
        check(TypingSource.resolve(KeyboardScheme.ZIRANMA, false, null) == TypingSource.ZIRANMA);
        check(TypingSource.resolve(KeyboardScheme.MICROSOFT, false, null) == TypingSource.MICROSOFT);
        check(TypingSource.resolve(KeyboardScheme.SHOUDAO, false, null) == TypingSource.SHOUDAO);
        check(TypingSource.resolve(KeyboardScheme.WUBI, false, null) == TypingSource.WUBI);
        check(TypingSource.resolve(KeyboardScheme.JAPANESE_NINE_KEY, false, null) == TypingSource.JAPANESE);
        check(TypingSource.resolve(KeyboardScheme.KOREAN, false, null) == TypingSource.KOREAN);
        check(TypingSource.resolve(KeyboardScheme.KOREAN, true, null) == TypingSource.ENGLISH);
        check(TypingSource.KOREAN.id().equals("korean"));
        check(TypingSource.resolve(KeyboardScheme.CANTONESE, false, null) == TypingSource.CANTONESE
            && TypingSource.CANTONESE.id().equals("cantonese"));
        check(TypingSource.resolve(KeyboardScheme.ZHUYIN, false, null) == TypingSource.ZHUYIN
            && TypingSource.ZHUYIN.id().equals("zhuyin"));
        check(TypingSource.resolve(KeyboardScheme.VIETNAMESE, false, null) == TypingSource.VIETNAMESE
            && TypingSource.VIETNAMESE.id().equals("vietnamese"));
        check(TypingSource.resolve(KeyboardScheme.VIETNAMESE, true, null) == TypingSource.ENGLISH);
        check(TypingSource.resolve(KeyboardScheme.TIBETAN, false, null) == TypingSource.TIBETAN
            && TypingSource.TIBETAN.id().equals("tibetan"));
        check(TypingSource.resolve(KeyboardScheme.TIBETAN, true, null) == TypingSource.ENGLISH);
        check(TypingSource.resolve(KeyboardScheme.HANDWRITING, false, null) == TypingSource.HANDWRITING);
        check(TypingSource.resolve(KeyboardScheme.QUANPIN, true, null) == TypingSource.ENGLISH);
        check(TypingSource.resolve(KeyboardScheme.QUANPIN, true, "emoji") == TypingSource.LOCAL);
        check(TypingSource.resolve(KeyboardScheme.QUANPIN, false, "temporary_japanese") == TypingSource.JAPANESE);
        check(TypingSource.AI.id().equals("ai") && TypingSource.REPLY.id().equals("reply")
            && TypingSource.VOICE.id().equals("voice"));
        System.out.println("Android typing statistics: Apple source mapping passed");
    }
}
