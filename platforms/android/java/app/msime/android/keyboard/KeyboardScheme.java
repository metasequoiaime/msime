package app.msime.android;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/** Shared-Engine keyboard schemes currently exposed by the Android host. */
public enum KeyboardScheme {
    QUANPIN("quanpin", "quanpin", null, "twenty_six_key", "全拼 26 键", "拼", "26"),
    QUANPIN_NINE_KEY("nine_key", "quanpin", null, "nine_key", "全拼 9 键", "拼", "9"),
    XIAOHE("xiaohe", "shuangpin", "xiaohe", "twenty_six_key", "小鹤双拼", "鹤", "双"),
    ZIRANMA("ziranma", "shuangpin", "ziranma", "twenty_six_key", "自然码双拼", "自", "双"),
    MICROSOFT("microsoft", "shuangpin", "microsoft", "twenty_six_key", "微软双拼", "微", "双"),
    SHOUDAO("shoudao", "shuangpin", "shoudao", "twenty_six_key", "首道双拼", "S", "双"),
    WUBI("wubi", "wubi", null, "twenty_six_key", "86 五笔", "五", "86"),
    JAPANESE_NINE_KEY("japanese_nine_key", "japanese", null, "nine_key", "日语 9 键", "あ", "9"),
    JAPANESE("japanese", "japanese", null, "twenty_six_key", "日语 26 键", "あ", "26"),
    HANDWRITING("handwriting", "quanpin", null, "handwriting", "手写", "写", "手"),
    KOREAN("korean", "korean", null, "twenty_six_key", "韩语 26 键", "한", "26"),
    CANTONESE("cantonese", "cantonese", null, "twenty_six_key", "粤拼 26 键", "粤", "26"),
    ZHUYIN("zhuyin", "zhuyin", null, "twenty_six_key", "大千注音", "注", "大千"),
    VIETNAMESE("vietnamese", "vietnamese", null, "twenty_six_key", "越南语 26 键", "越", "26"),
    TIBETAN("tibetan", "tibetan", null, "twenty_six_key", "藏文 26 键", "藏", "26"),
    // 笔画方案自己画五笔画键盘，偏好里的 26 键/9 键都显示它；与注音一样存 `twenty_six_key`，由宿主按方案号换面。
    STROKE("stroke", "stroke", null, "twenty_six_key", "笔画", "笔", "5"),
    // 注音 9 键与大千注音是同一个 Engine 方案，只是触屏布局存 `nine_key`；追加在末尾，与共享 `TouchKeyboardScheme::ALL` 的顺序一致。
    ZHUYIN_NINE_KEY("zhuyin_nine_key", "zhuyin", null, "nine_key", "注音 9 键", "注", "9");

    /** Complete preference values needed for one compare-and-swap update. */
    public record PreferenceMapping(
        String scheme, String lastChineseScheme, String shuangpinProfile,
        String touchKeyboardLayout) {}

    private final String preferenceId;
    private final String engineScheme;
    private final String shuangpinProfile;
    private final String touchKeyboardLayout;
    private final String title;
    private final String glyph;
    private final String badge;

    KeyboardScheme(String preferenceId, String engineScheme, String shuangpinProfile,
                   String touchKeyboardLayout, String title, String glyph, String badge) {
        this.preferenceId = preferenceId;
        this.engineScheme = engineScheme;
        this.shuangpinProfile = shuangpinProfile;
        this.touchKeyboardLayout = touchKeyboardLayout;
        this.title = title;
        this.glyph = glyph;
        this.badge = badge;
    }

    public String preferenceId() { return preferenceId; }

    /**
     * 选中这个入口时写进偏好 `scheme` 的 Engine 方案。
     *
     * <p>手写写的是本版本的默认方案（full 和拼音版是全拼，五笔版是五笔）：手写识别由平台识别器完成，不经过 Engine 的方案，手写面板背后的 Engine 只需要跑一个本版本提供的方案，否则 host-api 会把它当作本版本不含的方案回退，偏好里记的和 Engine 跑的就对不上了。
     */
    public String engineScheme(AppEdition edition) {
        return this == HANDWRITING ? edition.defaultScheme() : engineScheme;
    }

    /** 本版本的键盘是否提供这个入口：入口背后的方案在本版本里时提供。手写由 ML Kit 的 `zh-Hani-CN` 模型识别，只认汉字，所以只在提供中文方案的版本里有（full、拼音版、五笔版），日文、越南文和藏文版没有。与 client-core 的 `Edition::offers_touch_scheme` 一致。 */
    public boolean offeredBy(AppEdition edition) {
        if (this != HANDWRITING) return edition.offers(engineScheme);
        for (KeyboardScheme candidate : values()) {
            if (candidate != HANDWRITING && isChineseScheme(candidate.engineScheme)
                    && edition.offers(candidate.engineScheme)) return true;
        }
        return false;
    }

    /** 偏好里的方案本版本没有、或一个入口都没剩下时退回的入口：本版本提供全拼时是全拼 26 键（与引入版本之前相同），否则是本版本默认方案的 26 键入口（五笔版是五笔）。 */
    public static KeyboardScheme fallback(AppEdition edition) {
        if (QUANPIN.offeredBy(edition)) return QUANPIN;
        for (KeyboardScheme candidate : values()) {
            if (candidate != HANDWRITING && candidate.offeredBy(edition)
                    && candidate.engineScheme.equals(edition.defaultScheme())
                    && "twenty_six_key".equals(candidate.touchKeyboardLayout)) return candidate;
        }
        for (KeyboardScheme candidate : values()) {
            if (candidate != HANDWRITING && candidate.offeredBy(edition)) return candidate;
        }
        return HANDWRITING;
    }
    public String shuangpinProfile() { return shuangpinProfile; }
    public String touchKeyboardLayout() { return touchKeyboardLayout; }
    public String title() { return title; }
    public String glyph() { return glyph; }
    public String badge() { return badge; }

    /** 偏好 `wubi_profile` 的两个取值；方案仍是 `wubi`，版本是它旁边的独立字段，与 `shuangpin_profile` 之于 `shuangpin` 一样。 */
    public static final String WUBI_86 = "wubi86";
    public static final String WUBI_98 = "wubi98";

    /** 只认 `wubi98`，其它（缺省、未知值）一律按 86 版，与共享 `Preferences` 的缺省一致。 */
    public static String normalizedWubiProfile(String value) {
        return WUBI_98.equals(value) ? WUBI_98 : WUBI_86;
    }

    /** 方案名：五笔只有一个方案入口，标题跟随 `wubi_profile` 显示「86 五笔」或「98 五笔」；其它方案与 `title()` 相同。 */
    public String title(String wubiProfile) {
        if (this == WUBI && WUBI_98.equals(normalizedWubiProfile(wubiProfile))) return "98 五笔";
        return title;
    }

    /** 角标：五笔跟随 `wubi_profile` 显示「86」或「98」；其它方案与 `badge()` 相同。 */
    public String badge(String wubiProfile) {
        if (this == WUBI && WUBI_98.equals(normalizedWubiProfile(wubiProfile))) return "98";
        return badge;
    }

    /** 粤拼、注音（大千和 9 键）、越南语、藏文和笔画默认隐藏，用户打开后才出现；共享的 `TouchKeyboardScheme::DEFAULT_ENABLED` 同样不把它们放进从未存过列表的文档。 */
    public boolean optIn() {
        return this == CANTONESE || this == ZHUYIN || this == ZHUYIN_NINE_KEY || this == VIETNAMESE
            || this == TIBETAN || this == STROKE;
    }

    /** The file this scheme reads from the HostOptions `language_dictionaries` directory, or null for a scheme that needs only the shared resources. */
    public String languageDictionary() {
        if (this == CANTONESE) return "msime-cantonese.db";
        if (this == ZHUYIN || this == ZHUYIN_NINE_KEY) return "msime-zhuyin.db";
        if (this == STROKE) return "msime-stroke.db";
        return null;
    }

    /** Whether this scheme can run with the HostOptions `language_dictionaries` directory `directory`: without its dictionary host-api falls back from Cantonese, Zhuyin or Stroke, so offering the scheme would offer a keyboard that never takes effect. */
    public boolean installed(String directory) {
        String dictionary = languageDictionary();
        if (dictionary == null) return true;
        if (directory == null || directory.isEmpty()) return false;
        java.io.File root = new java.io.File(directory);
        return root.isAbsolute() && new java.io.File(root, dictionary).isFile();
    }

    /** `enabled` 里本版本提供、词典也已装好的入口；一个都不剩时与没存过列表一样退回 {@link #fallback}。 */
    public static List<KeyboardScheme> installedOf(
            List<KeyboardScheme> enabled, String directory, AppEdition edition) {
        List<KeyboardScheme> installed = new ArrayList<>(enabled.size());
        for (KeyboardScheme candidate : enabled) {
            if (candidate.offeredBy(edition) && candidate.installed(directory)) installed.add(candidate);
        }
        return withFallback(installed, edition);
    }

    public static KeyboardScheme fromPreferenceId(String value) {
        if (value == null) return null;
        for (KeyboardScheme candidate : values()) {
            if (candidate.preferenceId.equals(value)) return candidate;
        }
        return null;
    }

    /** 按固定顺序解析偏好里的入口 id，忽略不认识的和重复的，也忽略本版本没有的入口。没存过列表时，需要用户自己打开的那几个不启用；只有一个方案的版本例外，越南文版、藏文版的入口就是这个版本本身，与 client-core 的 `TouchKeyboardSchemePreferences::for_edition` 一致。 */
    public static List<KeyboardScheme> enabledFromPreferenceIds(List<String> ids, AppEdition edition) {
        if (ids == null) {
            List<KeyboardScheme> defaults = new ArrayList<>(values().length);
            for (KeyboardScheme candidate : values()) {
                if ((!candidate.optIn() || !edition.offersSchemeChoice()) && candidate.offeredBy(edition))
                    defaults.add(candidate);
            }
            return withFallback(defaults, edition);
        }
        Set<String> requested = new LinkedHashSet<>(ids);
        // A plain loop, not `Stream#toList`: that arrived in API 34 and this host declares
        // minSdk 28, so it compiles against the platform jar and throws on the device.
        List<KeyboardScheme> enabled = new ArrayList<>(values().length);
        for (KeyboardScheme candidate : values()) {
            if (requested.contains(candidate.preferenceId) && candidate.offeredBy(edition)) enabled.add(candidate);
        }
        return withFallback(enabled, edition);
    }

    private static List<KeyboardScheme> withFallback(List<KeyboardScheme> schemes,
            AppEdition edition) {
        return List.copyOf(availableOrFallback(schemes, edition));
    }

    private static List<KeyboardScheme> availableOrFallback(List<KeyboardScheme> schemes,
            AppEdition edition) {
        return schemes == null || schemes.isEmpty() ? List.of(fallback(edition)) : schemes;
    }

    /** Shared selected is authoritative; otherwise preserve the applied scheme or use first enabled. */
    public static KeyboardScheme resolveEnabledSelection(KeyboardScheme applied,
            String selectedPreferenceId, List<KeyboardScheme> enabled, AppEdition edition) {
        List<KeyboardScheme> available = availableOrFallback(enabled, edition);
        KeyboardScheme selected = fromPreferenceId(selectedPreferenceId);
        if (selected != null && available.contains(selected)) return selected;
        if (selectedPreferenceId == null && applied != null && available.contains(applied)) return applied;
        return available.get(0);
    }

    /** Returns the Engine preference mapping needed when the shared picker changed the fallback. */
    public static PreferenceMapping mappingForRuntimeSelection(
            KeyboardScheme applied, KeyboardScheme selected,
            String currentLastChineseScheme, String currentProfile, AppEdition edition) {
        if (selected == null || selected == applied) return null;
        return selected.mapping(currentLastChineseScheme, currentProfile, edition);
    }

    /** 偏好里的方案、双拼方案和触屏布局对应的入口；本版本没有那个入口时是 {@link #fallback}，与 host-api 把本版本不含的方案回退到默认方案一致。 */
    public static KeyboardScheme fromPreferences(
            String scheme, String profile, String touchLayout, AppEdition edition) {
        KeyboardScheme resolved = fromPreferences(scheme, profile, touchLayout, edition.defaultScheme());
        return resolved.offeredBy(edition) ? resolved : fallback(edition);
    }

    private static KeyboardScheme fromPreferences(
            String scheme, String profile, String touchLayout, String handwritingScheme) {
        if (handwritingScheme.equals(scheme) && "handwriting".equals(touchLayout)) return HANDWRITING;
        if ("quanpin".equals(scheme) && "nine_key".equals(touchLayout)) return QUANPIN_NINE_KEY;
        if ("japanese".equals(scheme) && "nine_key".equals(touchLayout)) return JAPANESE_NINE_KEY;
        if ("zhuyin".equals(scheme) && "nine_key".equals(touchLayout)) return ZHUYIN_NINE_KEY;
        if ("shuangpin".equals(scheme)) {
            for (KeyboardScheme candidate : values()) {
                if (profile != null && profile.equals(candidate.shuangpinProfile)) return candidate;
            }
            return XIAOHE;
        }
        for (KeyboardScheme candidate : values()) {
            if (candidate.shuangpinProfile == null && candidate.engineScheme.equals(scheme)
                    && !"nine_key".equals(candidate.touchKeyboardLayout)) return candidate;
        }
        return QUANPIN;
    }

    public PreferenceMapping mapping(
            String currentLastChineseScheme, String currentProfile, AppEdition edition) {
        String profile = normalizedProfile(currentProfile);
        if (shuangpinProfile != null) profile = shuangpinProfile;
        String scheme = engineScheme(edition);
        // 要切回的中文方案也只能是本版本有的；记着的那个本版本没有时，用本版本的默认方案（它不是中文方案时仍是全拼，与引入版本之前相同）。
        String lastChinese = isChineseScheme(currentLastChineseScheme) && edition.offers(currentLastChineseScheme)
            ? currentLastChineseScheme
            : isChineseScheme(edition.defaultScheme()) ? edition.defaultScheme() : "quanpin";
        // 日语、韩语、越南语和藏文保留要切回的中文方案，它们自己都不是中文方案；粤拼、注音和笔画是中文方案，会成为要切回的那个。
        if (isChineseScheme(scheme)) lastChinese = scheme;
        return new PreferenceMapping(scheme, lastChinese, profile, touchKeyboardLayout);
    }

    private static boolean isChineseScheme(String value) {
        return "quanpin".equals(value) || "shuangpin".equals(value) || "wubi".equals(value)
            || "cantonese".equals(value) || "zhuyin".equals(value) || "stroke".equals(value);
    }

    private static String normalizedProfile(String value) {
        if ("ziranma".equals(value) || "microsoft".equals(value) || "shoudao".equals(value)) {
            return value;
        }
        return "xiaohe";
    }
}
