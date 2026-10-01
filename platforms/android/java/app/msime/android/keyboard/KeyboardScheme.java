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
    THOUGHTFUL_REPLY("thoughtful_reply", "quanpin", null, "twenty_six_key", "高情商回复", "聊", "AI"),
    KOREAN("korean", "korean", null, "twenty_six_key", "韩语 26 键", "한", "26"),
    CANTONESE("cantonese", "cantonese", null, "twenty_six_key", "粤拼 26 键", "粤", "26"),
    ZHUYIN("zhuyin", "zhuyin", null, "twenty_six_key", "大千注音", "注", "大千"),
    VIETNAMESE("vietnamese", "vietnamese", null, "twenty_six_key", "越南语 26 键", "越", "26");

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
    public String engineScheme() { return engineScheme; }
    public String shuangpinProfile() { return shuangpinProfile; }
    public String touchKeyboardLayout() { return touchKeyboardLayout; }
    public String title() { return title; }
    public String glyph() { return glyph; }
    public String badge() { return badge; }

    /** Cantonese, Zhuyin and Vietnamese start hidden and appear once the user turns them on, as the shared `TouchKeyboardScheme::DEFAULT_ENABLED` keeps them out of a document that never stored a list. */
    public boolean optIn() {
        return this == CANTONESE || this == ZHUYIN || this == VIETNAMESE;
    }

    /** The file this scheme reads from the HostOptions `language_dictionaries` directory, or null for a scheme that needs only the shared resources. */
    public String languageDictionary() {
        if (this == CANTONESE) return "cantonese.db";
        if (this == ZHUYIN) return "zhuyin.db";
        return null;
    }

    /** Whether this scheme can run with the HostOptions `language_dictionaries` directory `directory`: without its dictionary host-api falls back from Cantonese or Zhuyin, so offering the scheme would offer a keyboard that never takes effect. */
    public boolean installed(String directory) {
        String dictionary = languageDictionary();
        if (dictionary == null) return true;
        if (directory == null || directory.isEmpty()) return false;
        java.io.File root = new java.io.File(directory);
        return root.isAbsolute() && new java.io.File(root, dictionary).isFile();
    }

    /** `enabled` without the schemes whose dictionary `directory` lacks, falling back to 全拼 26 键 like an empty stored list. */
    public static List<KeyboardScheme> installedOf(List<KeyboardScheme> enabled, String directory) {
        List<KeyboardScheme> installed = new ArrayList<>();
        for (KeyboardScheme candidate : enabled) {
            if (candidate.installed(directory)) installed.add(candidate);
        }
        return installed.isEmpty() ? List.of(QUANPIN) : List.copyOf(installed);
    }

    public static KeyboardScheme fromPreferenceId(String value) {
        if (value == null) return null;
        for (KeyboardScheme candidate : values()) {
            if (candidate.preferenceId.equals(value)) return candidate;
        }
        return null;
    }

    /** Resolves preference IDs in the fixed Apple order and ignores unknown duplicates. Without a stored list the opt-in schemes stay off. */
    public static List<KeyboardScheme> enabledFromPreferenceIds(List<String> ids) {
        if (ids == null) {
            List<KeyboardScheme> defaults = new ArrayList<>();
            for (KeyboardScheme candidate : values()) {
                if (!candidate.optIn()) defaults.add(candidate);
            }
            return List.copyOf(defaults);
        }
        Set<String> requested = new LinkedHashSet<>(ids);
        // A plain loop, not `Stream#toList`: that arrived in API 34 and this host declares
        // minSdk 28, so it compiles against the platform jar and throws on the device.
        List<KeyboardScheme> enabled = new ArrayList<>();
        for (KeyboardScheme candidate : values()) {
            if (requested.contains(candidate.preferenceId)) enabled.add(candidate);
        }
        return enabled.isEmpty() ? List.of(QUANPIN) : List.copyOf(enabled);
    }

    /** Shared selected is authoritative; otherwise preserve the applied scheme or use first enabled. */
    public static KeyboardScheme resolveEnabledSelection(
            KeyboardScheme applied, String selectedPreferenceId, List<KeyboardScheme> enabled) {
        List<KeyboardScheme> available = enabled == null || enabled.isEmpty()
            ? List.of(QUANPIN) : enabled;
        KeyboardScheme selected = fromPreferenceId(selectedPreferenceId);
        if (selected != null && available.contains(selected)) return selected;
        if (selectedPreferenceId == null && applied != null && available.contains(applied)) return applied;
        return available.get(0);
    }

    /** Returns the Engine preference mapping needed when the shared picker changed the fallback. */
    public static PreferenceMapping mappingForRuntimeSelection(
            KeyboardScheme applied, KeyboardScheme selected,
            String currentLastChineseScheme, String currentProfile) {
        if (selected == null || selected == THOUGHTFUL_REPLY || selected == applied) return null;
        return selected.mapping(currentLastChineseScheme, currentProfile);
    }

    public static KeyboardScheme fromHostSelection(String value, boolean thoughtfulEnabled,
                                                    KeyboardScheme engineSelection) {
        if (thoughtfulEnabled && THOUGHTFUL_REPLY.name().equals(value)
                && engineSelection == QUANPIN) return THOUGHTFUL_REPLY;
        return engineSelection;
    }

    public static KeyboardScheme fromPreferences(String scheme, String profile, String touchLayout) {
        if ("quanpin".equals(scheme) && "handwriting".equals(touchLayout)) return HANDWRITING;
        if ("quanpin".equals(scheme) && "nine_key".equals(touchLayout)) return QUANPIN_NINE_KEY;
        if ("japanese".equals(scheme) && "nine_key".equals(touchLayout)) return JAPANESE_NINE_KEY;
        if ("shuangpin".equals(scheme)) {
            for (KeyboardScheme candidate : values()) {
                if (profile != null && profile.equals(candidate.shuangpinProfile)) return candidate;
            }
            return XIAOHE;
        }
        for (KeyboardScheme candidate : values()) {
            if (candidate != THOUGHTFUL_REPLY && candidate.shuangpinProfile == null && candidate.engineScheme.equals(scheme)
                    && !"nine_key".equals(candidate.touchKeyboardLayout)) return candidate;
        }
        return QUANPIN;
    }

    public PreferenceMapping mapping(String currentLastChineseScheme, String currentProfile) {
        String profile = normalizedProfile(currentProfile);
        if (shuangpinProfile != null) profile = shuangpinProfile;
        String lastChinese = isChineseScheme(currentLastChineseScheme)
            ? currentLastChineseScheme : "quanpin";
        // Japanese, Korean and Vietnamese keep the Chinese scheme to return to; none is one. Cantonese and Zhuyin are Chinese schemes and become it.
        if (isChineseScheme(engineScheme)) lastChinese = engineScheme;
        return new PreferenceMapping(engineScheme, lastChinese, profile, touchKeyboardLayout);
    }

    private static boolean isChineseScheme(String value) {
        return "quanpin".equals(value) || "shuangpin".equals(value) || "wubi".equals(value)
            || "cantonese".equals(value) || "zhuyin".equals(value);
    }

    private static String normalizedProfile(String value) {
        if ("ziranma".equals(value) || "microsoft".equals(value) || "shoudao".equals(value)) {
            return value;
        }
        return "xiaohe";
    }
}
