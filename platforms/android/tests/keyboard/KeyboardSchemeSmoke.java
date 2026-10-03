import app.msime.android.KeyboardScheme;
import java.nio.file.Files;
import java.nio.file.Path;
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

    public static void main(String[] args) throws Exception {
        check(Arrays.stream(KeyboardScheme.values()).map(KeyboardScheme::title).toList().equals(List.of(
            "全拼 26 键", "全拼 9 键", "小鹤双拼", "自然码双拼", "微软双拼", "首道双拼", "86 五笔", "日语 9 键", "日语 26 键", "手写", "韩语 26 键",
            "粤拼 26 键", "大千注音", "越南语 26 键", "笔画")));
        check(Arrays.stream(KeyboardScheme.values()).map(KeyboardScheme::preferenceId).toList().equals(List.of(
            "quanpin", "nine_key", "xiaohe", "ziranma", "microsoft", "shoudao", "wubi",
            "japanese_nine_key", "japanese", "handwriting", "korean",
            "cantonese", "zhuyin", "vietnamese", "stroke")));
        check(Arrays.stream(KeyboardScheme.values()).map(value -> value.glyph() + value.badge()).toList().equals(
            List.of("拼26", "拼9", "鹤双", "自双", "微双", "S双", "五86", "あ9", "あ26", "写手", "한26",
                "粤26", "注大千", "越26", "笔5")));
        // 五笔只有一个方案入口，标题与角标跟随 `wubi_profile`；缺省和未知值按 86 版，其它方案不受影响。
        check(KeyboardScheme.normalizedWubiProfile("wubi98").equals("wubi98"));
        check(KeyboardScheme.normalizedWubiProfile("wubi86").equals("wubi86"));
        check(KeyboardScheme.normalizedWubiProfile(null).equals("wubi86"));
        check(KeyboardScheme.normalizedWubiProfile("WUBI98").equals("wubi86"));
        check(KeyboardScheme.WUBI.title("wubi98").equals("98 五笔") && KeyboardScheme.WUBI.badge("wubi98").equals("98"));
        check(KeyboardScheme.WUBI.title("wubi86").equals("86 五笔") && KeyboardScheme.WUBI.badge("wubi86").equals("86"));
        check(KeyboardScheme.WUBI.title(null).equals("86 五笔") && KeyboardScheme.WUBI.badge("future").equals("86"));
        check(KeyboardScheme.WUBI.glyph().equals("五"));
        for (KeyboardScheme value : KeyboardScheme.values()) {
            if (value == KeyboardScheme.WUBI) continue;
            check(value.title("wubi98").equals(value.title()) && value.badge("wubi98").equals(value.badge()));
        }
        check(KeyboardScheme.fromPreferenceId("japanese_nine_key") == KeyboardScheme.JAPANESE_NINE_KEY);
        check(KeyboardScheme.fromPreferenceId("korean") == KeyboardScheme.KOREAN);
        check(KeyboardScheme.fromPreferenceId("future") == null);
        List<KeyboardScheme> visible = KeyboardScheme.enabledFromPreferenceIds(List.of(
            "thoughtful_reply", "future", "nine_key", "nine_key", "quanpin"));
        check(visible.equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY)));
        // 高情商回复已不是输入方案，而是工具栏上的入口：旧偏好里存的 `thoughtful_reply` 与未知的未来方案一样被丢掉；只存了它的列表回落到全拼 26 键。
        check(KeyboardScheme.fromPreferenceId("thoughtful_reply") == null);
        check(Arrays.stream(KeyboardScheme.values()).noneMatch(value -> value.title().equals("高情商回复")));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("thoughtful_reply")).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of()).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, null, visible)
            == KeyboardScheme.QUANPIN_NINE_KEY);
        // 旧偏好选中的 `thoughtful_reply` 按未知方案处理，落到第一个启用的方案，与共享偏好读旧文档时的回落一致。
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "thoughtful_reply", visible)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "handwriting", visible)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.JAPANESE, KeyboardScheme.QUANPIN_NINE_KEY, "japanese", "xiaohe")
            .touchKeyboardLayout().equals("nine_key"));
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN, "quanpin", "xiaohe") == null);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "handwriting") == KeyboardScheme.HANDWRITING);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "nine_key") == KeyboardScheme.QUANPIN_NINE_KEY);
        check(KeyboardScheme.fromPreferences("japanese", "xiaohe", "nine_key") == KeyboardScheme.JAPANESE_NINE_KEY);
        check(KeyboardScheme.fromPreferences("korean", "xiaohe", "twenty_six_key") == KeyboardScheme.KOREAN);
        // Korean has one layout; a stale nine-key value must not fall back to another scheme.
        check(KeyboardScheme.fromPreferences("korean", "xiaohe", "nine_key") == KeyboardScheme.KOREAN);
        check(KeyboardScheme.fromPreferences("shuangpin", "microsoft", "nine_key") == KeyboardScheme.MICROSOFT);
        check(KeyboardScheme.fromPreferences("shuangpin", "unknown", "twenty_six_key") == KeyboardScheme.XIAOHE);
        check(KeyboardScheme.fromPreferences("future", "xiaohe", "nine_key") == KeyboardScheme.QUANPIN);
        // 98 五笔仍是 `scheme = wubi`，解析回同一个方案；映射只写四个键，切到五笔时 `wubi_profile` 原样保留。
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "twenty_six_key") == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "nine_key") == KeyboardScheme.WUBI);
        mapping(KeyboardScheme.QUANPIN, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        mapping(KeyboardScheme.QUANPIN_NINE_KEY, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        mapping(KeyboardScheme.XIAOHE, "quanpin", "shoudao", "shuangpin", "shuangpin", "xiaohe");
        mapping(KeyboardScheme.ZIRANMA, "quanpin", "xiaohe", "shuangpin", "shuangpin", "ziranma");
        mapping(KeyboardScheme.WUBI, "shuangpin", "ziranma", "wubi", "wubi", "ziranma");
        mapping(KeyboardScheme.JAPANESE, "wubi", "shoudao", "japanese", "wubi", "shoudao");
        mapping(KeyboardScheme.JAPANESE_NINE_KEY, "wubi", "shoudao", "japanese", "wubi", "shoudao");
        // Korean keeps the Chinese scheme to return to, as Japanese does, and is never one itself.
        mapping(KeyboardScheme.KOREAN, "wubi", "shoudao", "korean", "wubi", "shoudao");
        mapping(KeyboardScheme.KOREAN, "japanese", "xiaohe", "korean", "quanpin", "xiaohe");
        mapping(KeyboardScheme.KOREAN, "korean", "xiaohe", "korean", "quanpin", "xiaohe");
        mapping(KeyboardScheme.JAPANESE, "korean", "xiaohe", "japanese", "quanpin", "xiaohe");
        mapping(KeyboardScheme.HANDWRITING, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("korean", "quanpin"))
            .equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.KOREAN)));
        mapping(KeyboardScheme.JAPANESE, "invalid", "invalid", "japanese", "quanpin", "xiaohe");
        // Cantonese and Zhuyin are Chinese schemes and become the one to return to; Vietnamese keeps it, as Korean does.
        mapping(KeyboardScheme.CANTONESE, "wubi", "shoudao", "cantonese", "cantonese", "shoudao");
        mapping(KeyboardScheme.ZHUYIN, "wubi", "shoudao", "zhuyin", "zhuyin", "shoudao");
        mapping(KeyboardScheme.VIETNAMESE, "wubi", "shoudao", "vietnamese", "wubi", "shoudao");
        mapping(KeyboardScheme.VIETNAMESE, "zhuyin", "xiaohe", "vietnamese", "zhuyin", "xiaohe");
        mapping(KeyboardScheme.JAPANESE, "cantonese", "xiaohe", "japanese", "cantonese", "xiaohe");
        // Stroke is a Chinese scheme too: it becomes the one to return to, and a non-Chinese scheme keeps it.
        mapping(KeyboardScheme.STROKE, "wubi", "shoudao", "stroke", "stroke", "shoudao");
        mapping(KeyboardScheme.KOREAN, "stroke", "xiaohe", "korean", "stroke", "xiaohe");
        check(KeyboardScheme.fromPreferences("stroke", "xiaohe", "twenty_six_key") == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferences("stroke", "xiaohe", "nine_key") == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferenceId("stroke") == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferences("cantonese", "xiaohe", "twenty_six_key") == KeyboardScheme.CANTONESE);
        check(KeyboardScheme.fromPreferences("zhuyin", "xiaohe", "nine_key") == KeyboardScheme.ZHUYIN);
        check(KeyboardScheme.fromPreferences("vietnamese", "xiaohe", "twenty_six_key") == KeyboardScheme.VIETNAMESE);
        check(KeyboardScheme.fromPreferenceId("zhuyin") == KeyboardScheme.ZHUYIN);
        // Without a stored list the four newest schemes stay off; a stored list turns them on in the fixed order.
        List<KeyboardScheme> defaults = KeyboardScheme.enabledFromPreferenceIds(null);
        check(defaults.size() == 11 && !defaults.contains(KeyboardScheme.CANTONESE)
            && !defaults.contains(KeyboardScheme.ZHUYIN) && !defaults.contains(KeyboardScheme.VIETNAMESE)
            && !defaults.contains(KeyboardScheme.STROKE) && defaults.contains(KeyboardScheme.KOREAN));
        check(Arrays.stream(KeyboardScheme.values()).filter(KeyboardScheme::optIn).toList().equals(List.of(
            KeyboardScheme.CANTONESE, KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE, KeyboardScheme.STROKE)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("stroke", "quanpin")).equals(List.of(
            KeyboardScheme.QUANPIN, KeyboardScheme.STROKE)));
        List<KeyboardScheme> languages = KeyboardScheme.enabledFromPreferenceIds(List.of(
            "vietnamese", "zhuyin", "cantonese", "quanpin"));
        check(languages.equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.CANTONESE,
            KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE)));
        // A scheme is offered only when its dictionary is in the recorded directory; Vietnamese needs none.
        check(KeyboardScheme.CANTONESE.languageDictionary().equals("cantonese.db"));
        check(KeyboardScheme.ZHUYIN.languageDictionary().equals("zhuyin.db"));
        check(KeyboardScheme.STROKE.languageDictionary().equals("stroke.db"));
        check(!KeyboardScheme.STROKE.installed("") && !KeyboardScheme.STROKE.installed(null));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.STROKE), "").equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.VIETNAMESE.languageDictionary() == null && KeyboardScheme.QUANPIN.languageDictionary() == null);
        check(KeyboardScheme.VIETNAMESE.installed("") && KeyboardScheme.KOREAN.installed(null));
        check(!KeyboardScheme.CANTONESE.installed("") && !KeyboardScheme.ZHUYIN.installed(null));
        check(KeyboardScheme.installedOf(languages, "").equals(List.of(
            KeyboardScheme.QUANPIN, KeyboardScheme.VIETNAMESE)));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.ZHUYIN), "").equals(List.of(KeyboardScheme.QUANPIN)));
        Path directory = Files.createTempDirectory("msime-language-dictionaries");
        try {
            Files.write(directory.resolve("zhuyin.db"), new byte[] {1});
            String recorded = directory.toAbsolutePath().toString();
            check(KeyboardScheme.ZHUYIN.installed(recorded) && !KeyboardScheme.CANTONESE.installed(recorded)
                && !KeyboardScheme.STROKE.installed(recorded));
            check(!KeyboardScheme.ZHUYIN.installed("relative/" + directory.getFileName()));
            check(KeyboardScheme.installedOf(languages, recorded).equals(List.of(
                KeyboardScheme.QUANPIN, KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE)));
            check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "cantonese",
                KeyboardScheme.installedOf(languages, recorded)) == KeyboardScheme.QUANPIN);
            // stroke.db alone makes Stroke available and nothing else.
            Files.write(directory.resolve("stroke.db"), new byte[] {1});
            check(KeyboardScheme.STROKE.installed(recorded) && !KeyboardScheme.STROKE.installed("relative/stroke"));
            List<KeyboardScheme> withStroke = KeyboardScheme.enabledFromPreferenceIds(List.of(
                "stroke", "cantonese", "quanpin"));
            check(KeyboardScheme.installedOf(withStroke, recorded).equals(List.of(
                KeyboardScheme.QUANPIN, KeyboardScheme.STROKE)));
            check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "stroke",
                KeyboardScheme.installedOf(withStroke, recorded)) == KeyboardScheme.STROKE);
        } finally {
            Files.deleteIfExists(directory.resolve("stroke.db"));
            Files.deleteIfExists(directory.resolve("zhuyin.db"));
            Files.deleteIfExists(directory);
        }
        System.out.println("Android keyboard schemes: fifteen labels, glyphs, wubi profile titles, opt-in defaults, installed dictionaries, host fallback and shared preference mappings passed");
    }
}
