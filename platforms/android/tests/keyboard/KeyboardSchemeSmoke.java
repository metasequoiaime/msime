import app.msime.android.KeyboardScheme;
import java.util.Arrays;
import java.util.List;

public final class KeyboardSchemeSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static void mapping(KeyboardScheme scheme, String currentLast, String currentProfile,
                        String expectedScheme, String expectedLast, String expectedProfile) {
        KeyboardScheme.PreferenceMapping value = scheme.mapping(currentLast, currentProfile);
        check(value.scheme().equals(expectedScheme));
        check(value.lastChineseScheme().equals(expectedLast));
        check(value.shuangpinProfile().equals(expectedProfile));
        String expectedLayout = scheme == KeyboardScheme.HANDWRITING ? "handwriting"
            : scheme == KeyboardScheme.QUANPIN_NINE_KEY || scheme == KeyboardScheme.JAPANESE_NINE_KEY
                ? "nine_key" : "twenty_six_key";
        check(value.touchKeyboardLayout().equals(expectedLayout));
    }

    public static void main(String[] args) {
        check(Arrays.stream(KeyboardScheme.values()).map(KeyboardScheme::title).toList().equals(List.of(
            "全拼 26 键", "全拼 9 键", "小鹤双拼", "自然码双拼", "微软双拼", "首道双拼", "86 五笔", "日语 9 键", "日语 26 键", "手写", "高情商回复")));
        check(Arrays.stream(KeyboardScheme.values()).map(KeyboardScheme::preferenceId).toList().equals(List.of(
            "quanpin", "nine_key", "xiaohe", "ziranma", "microsoft", "shoudao", "wubi",
            "japanese_nine_key", "japanese", "handwriting", "thoughtful_reply")));
        check(Arrays.stream(KeyboardScheme.values()).map(value -> value.glyph() + value.badge()).toList().equals(
            List.of("拼26", "拼9", "鹤双", "自双", "微双", "S双", "五86", "あ9", "あ26", "写手", "聊AI")));
        check(KeyboardScheme.fromPreferenceId("japanese_nine_key") == KeyboardScheme.JAPANESE_NINE_KEY);
        check(KeyboardScheme.fromPreferenceId("future") == null);
        List<KeyboardScheme> visible = KeyboardScheme.enabledFromPreferenceIds(List.of(
            "thoughtful_reply", "future", "nine_key", "nine_key", "quanpin"));
        check(visible.equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY,
            KeyboardScheme.THOUGHTFUL_REPLY)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of()).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, null, visible)
            == KeyboardScheme.QUANPIN_NINE_KEY);
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "thoughtful_reply", visible)
            == KeyboardScheme.THOUGHTFUL_REPLY);
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "handwriting", visible)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.JAPANESE, KeyboardScheme.QUANPIN_NINE_KEY, "japanese", "xiaohe")
            .touchKeyboardLayout().equals("nine_key"));
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.QUANPIN, KeyboardScheme.THOUGHTFUL_REPLY, "quanpin", "xiaohe") == null);
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN, "quanpin", "xiaohe") == null);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "handwriting") == KeyboardScheme.HANDWRITING);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "nine_key") == KeyboardScheme.QUANPIN_NINE_KEY);
        check(KeyboardScheme.fromPreferences("japanese", "xiaohe", "nine_key") == KeyboardScheme.JAPANESE_NINE_KEY);
        check(KeyboardScheme.fromPreferences("shuangpin", "microsoft", "nine_key") == KeyboardScheme.MICROSOFT);
        check(KeyboardScheme.fromPreferences("shuangpin", "unknown", "twenty_six_key") == KeyboardScheme.XIAOHE);
        check(KeyboardScheme.fromPreferences("future", "xiaohe", "nine_key") == KeyboardScheme.QUANPIN);
        mapping(KeyboardScheme.QUANPIN, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        mapping(KeyboardScheme.QUANPIN_NINE_KEY, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        mapping(KeyboardScheme.XIAOHE, "quanpin", "shoudao", "shuangpin", "shuangpin", "xiaohe");
        mapping(KeyboardScheme.ZIRANMA, "quanpin", "xiaohe", "shuangpin", "shuangpin", "ziranma");
        mapping(KeyboardScheme.WUBI, "shuangpin", "ziranma", "wubi", "wubi", "ziranma");
        mapping(KeyboardScheme.JAPANESE, "wubi", "shoudao", "japanese", "wubi", "shoudao");
        mapping(KeyboardScheme.JAPANESE_NINE_KEY, "wubi", "shoudao", "japanese", "wubi", "shoudao");
        mapping(KeyboardScheme.HANDWRITING, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        mapping(KeyboardScheme.THOUGHTFUL_REPLY, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        check(KeyboardScheme.fromHostSelection("THOUGHTFUL_REPLY", true, KeyboardScheme.QUANPIN)
            == KeyboardScheme.THOUGHTFUL_REPLY);
        check(KeyboardScheme.fromHostSelection("THOUGHTFUL_REPLY", false, KeyboardScheme.QUANPIN)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.fromHostSelection("THOUGHTFUL_REPLY", true, KeyboardScheme.WUBI)
            == KeyboardScheme.WUBI);
        mapping(KeyboardScheme.JAPANESE, "invalid", "invalid", "japanese", "quanpin", "xiaohe");
        System.out.println("Android keyboard schemes: eleven labels, glyphs, host fallback and shared preference mappings passed");
    }
}
