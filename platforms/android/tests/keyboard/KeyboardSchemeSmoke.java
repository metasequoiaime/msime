import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;

public final class KeyboardSchemeSmoke {
    static final AppEdition FULL = AppEdition.FULL;
    static final AppEdition PINYIN = AppEdition.of("pinyin", "quanpin,shuangpin", "quanpin", true);
    static final AppEdition WUBI = AppEdition.of("wubi", "wubi", "wubi", false);

    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    static void mapping(KeyboardScheme scheme, String currentLast, String currentProfile,
                        String expectedScheme, String expectedLast, String expectedProfile) {
        mapping(FULL, scheme, currentLast, currentProfile, expectedScheme, expectedLast, expectedProfile);
    }

    static void mapping(AppEdition edition, KeyboardScheme scheme, String currentLast, String currentProfile,
                        String expectedScheme, String expectedLast, String expectedProfile) {
        KeyboardScheme.PreferenceMapping value = scheme.mapping(currentLast, currentProfile, edition);
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
            "粤拼 26 键", "大千注音", "越南语 26 键", "藏文 26 键", "笔画")));
        check(Arrays.stream(KeyboardScheme.values()).map(KeyboardScheme::preferenceId).toList().equals(List.of(
            "quanpin", "nine_key", "xiaohe", "ziranma", "microsoft", "shoudao", "wubi",
            "japanese_nine_key", "japanese", "handwriting", "korean",
            "cantonese", "zhuyin", "vietnamese", "tibetan", "stroke")));
        check(Arrays.stream(KeyboardScheme.values()).map(value -> value.glyph() + value.badge()).toList().equals(
            List.of("拼26", "拼9", "鹤双", "自双", "微双", "S双", "五86", "あ9", "あ26", "写手", "한26",
                "粤26", "注大千", "越26", "藏26", "笔5")));
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
            "thoughtful_reply", "future", "nine_key", "nine_key", "quanpin"), FULL);
        check(visible.equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY)));
        // 高情商回复已不是输入方案，而是工具栏上的入口：旧偏好里存的 `thoughtful_reply` 与未知的未来方案一样被丢掉；只存了它的列表回落到全拼 26 键。
        check(KeyboardScheme.fromPreferenceId("thoughtful_reply") == null);
        check(Arrays.stream(KeyboardScheme.values()).noneMatch(value -> value.title().equals("高情商回复")));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("thoughtful_reply"), FULL).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of(), FULL).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, null, visible, FULL)
            == KeyboardScheme.QUANPIN_NINE_KEY);
        // 旧偏好选中的 `thoughtful_reply` 按未知方案处理，落到第一个启用的方案，与共享偏好读旧文档时的回落一致。
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "thoughtful_reply", visible, FULL)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN_NINE_KEY, "handwriting", visible, FULL)
            == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.JAPANESE, KeyboardScheme.QUANPIN_NINE_KEY, "japanese", "xiaohe", FULL)
            .touchKeyboardLayout().equals("nine_key"));
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN, "quanpin", "xiaohe", FULL) == null);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "handwriting", FULL) == KeyboardScheme.HANDWRITING);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "nine_key", FULL) == KeyboardScheme.QUANPIN_NINE_KEY);
        check(KeyboardScheme.fromPreferences("japanese", "xiaohe", "nine_key", FULL) == KeyboardScheme.JAPANESE_NINE_KEY);
        check(KeyboardScheme.fromPreferences("korean", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.KOREAN);
        // Korean has one layout; a stale nine-key value must not fall back to another scheme.
        check(KeyboardScheme.fromPreferences("korean", "xiaohe", "nine_key", FULL) == KeyboardScheme.KOREAN);
        check(KeyboardScheme.fromPreferences("shuangpin", "microsoft", "nine_key", FULL) == KeyboardScheme.MICROSOFT);
        check(KeyboardScheme.fromPreferences("shuangpin", "unknown", "twenty_six_key", FULL) == KeyboardScheme.XIAOHE);
        check(KeyboardScheme.fromPreferences("future", "xiaohe", "nine_key", FULL) == KeyboardScheme.QUANPIN);
        // 98 五笔仍是 `scheme = wubi`，解析回同一个方案；映射只写四个键，切到五笔时 `wubi_profile` 原样保留。
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "nine_key", FULL) == KeyboardScheme.WUBI);
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
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("korean", "quanpin"), FULL)
            .equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.KOREAN)));
        mapping(KeyboardScheme.JAPANESE, "invalid", "invalid", "japanese", "quanpin", "xiaohe");
        // Cantonese and Zhuyin are Chinese schemes and become the one to return to; Vietnamese keeps it, as Korean does.
        mapping(KeyboardScheme.CANTONESE, "wubi", "shoudao", "cantonese", "cantonese", "shoudao");
        mapping(KeyboardScheme.ZHUYIN, "wubi", "shoudao", "zhuyin", "zhuyin", "shoudao");
        mapping(KeyboardScheme.VIETNAMESE, "wubi", "shoudao", "vietnamese", "wubi", "shoudao");
        mapping(KeyboardScheme.VIETNAMESE, "zhuyin", "xiaohe", "vietnamese", "zhuyin", "xiaohe");
        // 藏文和越南语一样不是中文方案，保留要切回的中文方案；记着的不是中文方案时回到全拼。
        mapping(KeyboardScheme.TIBETAN, "wubi", "shoudao", "tibetan", "wubi", "shoudao");
        mapping(KeyboardScheme.TIBETAN, "cantonese", "xiaohe", "tibetan", "cantonese", "xiaohe");
        mapping(KeyboardScheme.TIBETAN, "tibetan", "xiaohe", "tibetan", "quanpin", "xiaohe");
        mapping(KeyboardScheme.KOREAN, "tibetan", "xiaohe", "korean", "quanpin", "xiaohe");
        mapping(KeyboardScheme.JAPANESE, "cantonese", "xiaohe", "japanese", "cantonese", "xiaohe");
        // Stroke is a Chinese scheme too: it becomes the one to return to, and a non-Chinese scheme keeps it.
        mapping(KeyboardScheme.STROKE, "wubi", "shoudao", "stroke", "stroke", "shoudao");
        mapping(KeyboardScheme.KOREAN, "stroke", "xiaohe", "korean", "stroke", "xiaohe");
        check(KeyboardScheme.fromPreferences("stroke", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferences("stroke", "xiaohe", "nine_key", FULL) == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferenceId("stroke") == KeyboardScheme.STROKE);
        check(KeyboardScheme.fromPreferences("cantonese", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.CANTONESE);
        check(KeyboardScheme.fromPreferences("zhuyin", "xiaohe", "nine_key", FULL) == KeyboardScheme.ZHUYIN);
        check(KeyboardScheme.fromPreferences("vietnamese", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.VIETNAMESE);
        check(KeyboardScheme.fromPreferences("tibetan", "xiaohe", "twenty_six_key", FULL) == KeyboardScheme.TIBETAN);
        // 藏文只有 26 键，残留的九键或手写取值不会落到别的方案。
        check(KeyboardScheme.fromPreferences("tibetan", "xiaohe", "nine_key", FULL) == KeyboardScheme.TIBETAN);
        check(KeyboardScheme.fromPreferenceId("tibetan") == KeyboardScheme.TIBETAN);
        check(KeyboardScheme.fromPreferenceId("zhuyin") == KeyboardScheme.ZHUYIN);
        // 没有存储列表时最新的五种方案保持关闭；存储了列表时按固定顺序打开它们。
        List<KeyboardScheme> defaults = KeyboardScheme.enabledFromPreferenceIds(null, FULL);
        check(defaults.size() == 11 && !defaults.contains(KeyboardScheme.CANTONESE)
            && !defaults.contains(KeyboardScheme.ZHUYIN) && !defaults.contains(KeyboardScheme.VIETNAMESE)
            && !defaults.contains(KeyboardScheme.TIBETAN) && !defaults.contains(KeyboardScheme.STROKE)
            && defaults.contains(KeyboardScheme.KOREAN));
        check(Arrays.stream(KeyboardScheme.values()).filter(KeyboardScheme::optIn).toList().equals(List.of(
            KeyboardScheme.CANTONESE, KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE, KeyboardScheme.TIBETAN,
            KeyboardScheme.STROKE)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("stroke", "quanpin"), FULL).equals(List.of(
            KeyboardScheme.QUANPIN, KeyboardScheme.STROKE)));
        List<KeyboardScheme> languages = KeyboardScheme.enabledFromPreferenceIds(List.of(
            "tibetan", "vietnamese", "zhuyin", "cantonese", "quanpin"), FULL);
        check(languages.equals(List.of(KeyboardScheme.QUANPIN, KeyboardScheme.CANTONESE,
            KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE, KeyboardScheme.TIBETAN)));
        // A scheme is offered only when its dictionary is in the recorded directory; Vietnamese needs none.
        check(KeyboardScheme.CANTONESE.languageDictionary().equals("cantonese.db"));
        check(KeyboardScheme.ZHUYIN.languageDictionary().equals("zhuyin.db"));
        check(KeyboardScheme.STROKE.languageDictionary().equals("stroke.db"));
        check(!KeyboardScheme.STROKE.installed("") && !KeyboardScheme.STROKE.installed(null));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.STROKE), "", FULL).equals(List.of(KeyboardScheme.QUANPIN)));
        check(KeyboardScheme.VIETNAMESE.languageDictionary() == null && KeyboardScheme.QUANPIN.languageDictionary() == null);
        check(KeyboardScheme.VIETNAMESE.installed("") && KeyboardScheme.KOREAN.installed(null));
        // 藏文同样不需要词库，没有记录词库目录时也提供。
        check(KeyboardScheme.TIBETAN.languageDictionary() == null && KeyboardScheme.TIBETAN.installed(null));
        check(!KeyboardScheme.CANTONESE.installed("") && !KeyboardScheme.ZHUYIN.installed(null));
        check(KeyboardScheme.installedOf(languages, "", FULL).equals(List.of(
            KeyboardScheme.QUANPIN, KeyboardScheme.VIETNAMESE, KeyboardScheme.TIBETAN)));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.ZHUYIN), "", FULL).equals(List.of(KeyboardScheme.QUANPIN)));
        Path directory = Files.createTempDirectory("msime-language-dictionaries");
        try {
            Files.write(directory.resolve("zhuyin.db"), new byte[] {1});
            String recorded = directory.toAbsolutePath().toString();
            check(KeyboardScheme.ZHUYIN.installed(recorded) && !KeyboardScheme.CANTONESE.installed(recorded)
                && !KeyboardScheme.STROKE.installed(recorded));
            check(!KeyboardScheme.ZHUYIN.installed("relative/" + directory.getFileName()));
            check(KeyboardScheme.installedOf(languages, recorded, FULL).equals(List.of(
                KeyboardScheme.QUANPIN, KeyboardScheme.ZHUYIN, KeyboardScheme.VIETNAMESE, KeyboardScheme.TIBETAN)));
            check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "cantonese",
                KeyboardScheme.installedOf(languages, recorded, FULL), FULL) == KeyboardScheme.QUANPIN);
            // stroke.db alone makes Stroke available and nothing else.
            Files.write(directory.resolve("stroke.db"), new byte[] {1});
            check(KeyboardScheme.STROKE.installed(recorded) && !KeyboardScheme.STROKE.installed("relative/stroke"));
            List<KeyboardScheme> withStroke = KeyboardScheme.enabledFromPreferenceIds(List.of(
                "stroke", "cantonese", "quanpin"), FULL);
            check(KeyboardScheme.installedOf(withStroke, recorded, FULL).equals(List.of(
                KeyboardScheme.QUANPIN, KeyboardScheme.STROKE)));
            check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.QUANPIN, "stroke",
                KeyboardScheme.installedOf(withStroke, recorded, FULL), FULL) == KeyboardScheme.STROKE);
        } finally {
            Files.deleteIfExists(directory.resolve("stroke.db"));
            Files.deleteIfExists(directory.resolve("zhuyin.db"));
            Files.deleteIfExists(directory);
        }
        editions();
        System.out.println("Android keyboard schemes: fifteen labels, glyphs, wubi profile titles, opt-in defaults, installed dictionaries, host fallback and shared preference mappings and the per-edition narrowing passed");
    }

    /** 五笔版和拼音版只列本版本的入口，回退也落在本版本里；手写在每个版本都有，写进偏好的是本版本的默认方案。 */
    static void editions() {
        // JVM 冒烟测试里没有 Gradle 生成的 BuildConfig，读到的就是 full。
        check(AppEdition.current() == FULL && FULL.isFull() && FULL.defaultScheme().equals("quanpin"));
        check(!WUBI.isFull() && !WUBI.offersSchemeChoice() && PINYIN.offersSchemeChoice());
        check(FULL.temporaryJapanese() && PINYIN.temporaryJapanese() && !WUBI.temporaryJapanese());
        try {
            AppEdition.of("wubi", "wubi", "quanpin", false);
            throw new IllegalStateException("a default scheme outside the edition must be rejected");
        } catch (IllegalArgumentException expected) {
            // 默认方案不在本版本的方案里，声明不成立。
        }
        check(Arrays.stream(KeyboardScheme.values()).allMatch(value -> value.offeredBy(FULL)));
        check(Arrays.stream(KeyboardScheme.values()).filter(value -> value.offeredBy(WUBI)).toList().equals(
            List.of(KeyboardScheme.WUBI, KeyboardScheme.HANDWRITING)));
        check(Arrays.stream(KeyboardScheme.values()).filter(value -> value.offeredBy(PINYIN)).toList().equals(
            List.of(KeyboardScheme.QUANPIN, KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.XIAOHE,
                KeyboardScheme.ZIRANMA, KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO, KeyboardScheme.HANDWRITING)));
        check(KeyboardScheme.fallback(FULL) == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.fallback(PINYIN) == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.fallback(WUBI) == KeyboardScheme.WUBI);
        // 五笔版没有存过列表时只有五笔和手写，全拼 9 键不在其中；存过的列表里本版本没有的入口一律丢掉。
        check(KeyboardScheme.enabledFromPreferenceIds(null, WUBI).equals(
            List.of(KeyboardScheme.WUBI, KeyboardScheme.HANDWRITING)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("quanpin", "nine_key", "japanese"), WUBI)
            .equals(List.of(KeyboardScheme.WUBI)));
        check(KeyboardScheme.enabledFromPreferenceIds(List.of("nine_key", "handwriting"), WUBI)
            .equals(List.of(KeyboardScheme.HANDWRITING)));
        check(KeyboardScheme.enabledFromPreferenceIds(null, PINYIN).equals(List.of(KeyboardScheme.QUANPIN,
            KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA,
            KeyboardScheme.MICROSOFT, KeyboardScheme.SHOUDAO, KeyboardScheme.HANDWRITING)));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.values()), "", WUBI).equals(
            List.of(KeyboardScheme.WUBI, KeyboardScheme.HANDWRITING)));
        check(KeyboardScheme.installedOf(List.of(KeyboardScheme.QUANPIN_NINE_KEY), "", WUBI).equals(
            List.of(KeyboardScheme.WUBI)));
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.WUBI, "nine_key",
            List.of(KeyboardScheme.WUBI, KeyboardScheme.HANDWRITING), WUBI) == KeyboardScheme.WUBI);
        check(KeyboardScheme.resolveEnabledSelection(KeyboardScheme.WUBI, null, List.of(), WUBI)
            == KeyboardScheme.WUBI);
        // 偏好里是本版本没有的方案（例如从别处带来的全拼九键）时，与 host-api 一样回退到本版本的默认方案。
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "nine_key", WUBI) == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "twenty_six_key", WUBI) == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("future", "xiaohe", "twenty_six_key", WUBI) == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "twenty_six_key", PINYIN) == KeyboardScheme.QUANPIN);
        check(KeyboardScheme.fromPreferences("shuangpin", "ziranma", "twenty_six_key", PINYIN) == KeyboardScheme.ZIRANMA);
        // 手写：五笔版写进偏好的是 `wubi` 加手写布局，读回来仍是手写；全拼加手写布局在五笔版里不是本版本的方案。
        check(KeyboardScheme.HANDWRITING.engineScheme(WUBI).equals("wubi"));
        check(KeyboardScheme.HANDWRITING.engineScheme(FULL).equals("quanpin"));
        check(KeyboardScheme.HANDWRITING.engineScheme(PINYIN).equals("quanpin"));
        check(KeyboardScheme.WUBI.engineScheme(FULL).equals("wubi"));
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "handwriting", WUBI) == KeyboardScheme.HANDWRITING);
        check(KeyboardScheme.fromPreferences("quanpin", "xiaohe", "handwriting", WUBI) == KeyboardScheme.WUBI);
        check(KeyboardScheme.fromPreferences("wubi", "xiaohe", "handwriting", FULL) == KeyboardScheme.WUBI);
        mapping(WUBI, KeyboardScheme.HANDWRITING, "wubi", "xiaohe", "wubi", "wubi", "xiaohe");
        mapping(WUBI, KeyboardScheme.HANDWRITING, "quanpin", "xiaohe", "wubi", "wubi", "xiaohe");
        mapping(WUBI, KeyboardScheme.WUBI, "quanpin", "xiaohe", "wubi", "wubi", "xiaohe");
        // 要切回的中文方案只取本版本有的：拼音版记着五笔时回到全拼。
        mapping(PINYIN, KeyboardScheme.HANDWRITING, "wubi", "microsoft", "quanpin", "quanpin", "microsoft");
        check(KeyboardScheme.mappingForRuntimeSelection(
            KeyboardScheme.WUBI, KeyboardScheme.HANDWRITING, "wubi", "xiaohe", WUBI).scheme().equals("wubi"));
    }
}
