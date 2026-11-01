package app.msime.android;

/**
 * 候选读音的取词规则，和桌面端相同：英文候选读它本身（英文），中文候选读释义的第一行，按释义的目标语言读。英文给 `/…/` 音标，法、西、俄、德、韩给维基词典的 IPA（`/…/` 或 `[…]`），日文给罗马字。
 *
 * <p>读音按「语言 + 换行 + 文字」这个键（{@link #key}）请求和缓存：同样的字母在别的语言里是另一个词（法文的 chat）。
 */
public final class CandidatePronunciationPolicy {
    private CandidatePronunciationPolicy() {}

    /** 共享层有读音表的语言，和 {@code msime_client_pronunciation_request} 认的一样。 */
    public static final java.util.Set<String> LANGUAGES = java.util.Set.of("en", "fr", "ja", "es", "ru", "de", "ko");

    /** 一段文字和它的语言合成的键；释义只取第一行，文字里不会有换行。 */
    public static String key(String language, String text) {
        return language + "\n" + text;
    }

    public static boolean visible(boolean enabled, boolean resourceAvailable) {
        return enabled && resourceAvailable;
    }

    /** 返回交给共享表的那一段的键（{@link #key}），没有可读的文字时为空串；释义的首个词组和去冠词规则由共享层统一处理。{@code target} 是释义的目标语言，共享层没有它的表时不查。 */
    public static String lookupText(String candidate, String translation, String target) {
        if (plainEnglish(candidate)) return key("en", candidate);
        if (target == null || !LANGUAGES.contains(target) || translation == null || translation.isEmpty()) return "";
        String first = translation.split("\\R", 2)[0].trim();
        return first.isEmpty() || hasControl(first) ? "" : key(target, first);
    }

    /** 读屏在释义之后念的读音：IPA 叫音标，日文的罗马字叫读音。 */
    public static String accessibilitySuffix(String reading) {
        if (reading == null || reading.isEmpty()) return "";
        return (reading.startsWith("/") || reading.startsWith("[") ? "，音标：" : "，读音：") + reading;
    }

    /** 共享层的请求只要有一段带控制字符（Rust 的 `char::is_control`）就整批拒绝；这样的释义（比如在线或 AI 译文里的制表符）不查，免得一条释义让整页都没有音标。 */
    private static boolean hasControl(String text) {
        for (int index = 0; index < text.length(); index++) {
            if (Character.isISOControl(text.charAt(index))) return true;
        }
        return false;
    }

    private static boolean plainEnglish(String text) {
        if (text == null || text.isEmpty()) return false;
        boolean letter = false;
        for (int index = 0; index < text.length(); index++) {
            char value = text.charAt(index);
            if (value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z') letter = true;
            else if (value != ' ' && value != '-' && value != '\'') return false;
        }
        return letter;
    }

    public static String withReading(String annotation, String reading) {
        String base = annotation == null ? "" : annotation;
        return reading == null || reading.isEmpty() ? base
            : base.isEmpty() ? reading : base + "\n" + reading;
    }

    /** 英文直输建议查询中保留第二行，开关关闭时恢复单行。 */
    public static String suggestionAnnotation(boolean visible, String reading) {
        return !visible ? "" : reading == null || reading.isEmpty() ? "\u00A0" : reading;
    }
}
