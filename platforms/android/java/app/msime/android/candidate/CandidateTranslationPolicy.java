package app.msime.android;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/** Validates and presents the one-or-two language candidate gloss configuration. */
public final class CandidateTranslationPolicy {
    private static final Set<String> SUPPORTED = Set.of("en", "fr", "ja", "es", "ru", "de", "ko");
    /** Targets with an offline dictionary format; mirrors OFFLINE_GLOSS_LANGUAGES in crates/host-api. */
    public static final Set<String> OFFLINE_GLOSS_LANGUAGES = Set.of("fr", "ja", "es", "ru", "de", "ko");

    private CandidateTranslationPolicy() {}

    /** Primary always falls back to English; a malformed or duplicate secondary is ignored. */
    public static List<String> targets(String primary, String secondary) {
        ArrayList<String> result = new ArrayList<>(2);
        String first = normalize(primary);
        result.add(SUPPORTED.contains(first) ? first : "en");
        String second = normalize(secondary);
        if (SUPPORTED.contains(second) && !result.contains(second)) result.add(second);
        return List.copyOf(result);
    }

    /** Keep the two language rows readable in a single Android candidate annotation. */
    public static String joinGlosses(List<String> glosses) {
        if (glosses == null || glosses.isEmpty()) return "";
        return String.join("\n", glosses);
    }

    /** Return the bounded, user-visible gloss rows that a long press may insert. */
    public static List<String> insertionGlosses(String translation) {
        if (translation == null || translation.isEmpty()) return List.of();
        ArrayList<String> result = new ArrayList<>(2);
        for (String value : translation.split("\\R", -1)) {
            String gloss = value.trim();
            if (gloss.isEmpty() || result.contains(gloss)
                    || TextPolicy.utf8Length(gloss) > 4096
                    || TextPolicy.hasControl(gloss)) continue;
            result.add(gloss);
            if (result.size() == 2) break;
        }
        return List.copyOf(result);
    }

    /**
     * Whether candidate words may be sent to the MSIME account endpoint (api.msime.app).
     *
     * <p>Only an explicit `translation_account` choice selects it, and a user's own NiuTrans or custom service always wins over it, so nothing is sent when the user never chose. This mirrors the `translation_account` rule in the shared core (`msime_client_translation_query` in `crates/host-api/src/ffi/providers.rs`) except for the Tencent clause: this host has no Tencent client and neither Android settings surface can enter Tencent credentials. The shared settings page also writes Tencent's `enabled` to false when the account is chosen; the native feature switch writes only `translation_account`. Plain booleans because the JVM smokes cannot load org.json.
     */
    public static boolean accountSelected(boolean candidateTranslations,
            boolean translationAccount, boolean niutransEnabled, boolean customEnabled) {
        return candidateTranslations && translationAccount && !niutransEnabled && !customEnabled;
    }

    /** Count rows that can actually be filled by the enabled offline/online paths. The offline switch covers English and every target in {@code offlineTargets}. */
    public static int glossLines(List<String> targets, boolean offline, boolean online,
            Collection<String> offlineTargets) {
        if (targets == null || targets.isEmpty()) return 0;
        int lines = 0;
        for (String target : targets) {
            String code = normalize(target);
            if (online || (offline && ("en".equals(code)
                    || (offlineTargets != null && offlineTargets.contains(code))))) lines++;
        }
        return lines;
    }

    /** The non-English targets whose dictionary is installed in the {@code offline-glosses} directory beside {@code resources}, in target order. */
    public static List<String> offlineTargets(List<String> targets, String resources) {
        if (targets == null || resources == null || resources.isEmpty()) return List.of();
        File parent = new File(resources).getParentFile();
        if (parent == null) return List.of();
        ArrayList<String> result = new ArrayList<>(2);
        for (String target : targets) {
            String code = normalize(target);
            if (OFFLINE_GLOSS_LANGUAGES.contains(code)
                    && new File(parent, "offline-glosses/zh-" + code + ".db").isFile()) result.add(code);
        }
        return List.copyOf(result);
    }

    /** One candidate's rows in target order: the offline gloss for a target first, the account translation where there is none. */
    public static String mergeGlosses(List<String> targets, Map<String, String> offline,
            Map<String, String> online) {
        if (targets == null) return "";
        ArrayList<String> glosses = new ArrayList<>(2);
        for (String target : targets) {
            String gloss = offline == null ? null : offline.get(target);
            if (gloss == null || gloss.isEmpty()) gloss = online == null ? null : online.get(target);
            if (gloss != null && !gloss.isEmpty()) glosses.add(gloss);
        }
        return joinGlosses(glosses);
    }

    /** Number of rows needed by one rendered candidate label, based on actual annotation text. */
    public static int renderedGlossLines(String annotation) {
        return annotation != null && annotation.indexOf('\n') >= 0 ? 2 : 1;
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim().toLowerCase(Locale.ROOT);
    }
}
