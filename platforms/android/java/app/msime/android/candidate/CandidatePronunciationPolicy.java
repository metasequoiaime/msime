package app.msime.android;

/** 英文候选和英语释义共用桌面端的离线宽式音标规则。 */
public final class CandidatePronunciationPolicy {
    private CandidatePronunciationPolicy() {}

    public static boolean visible(boolean enabled, boolean resourceAvailable) {
        return enabled && resourceAvailable;
    }

    /** 返回交给共享表的文字；释义的首个词组和去冠词规则由共享层统一处理。 */
    public static String lookupText(String candidate, String translation, boolean englishTarget) {
        if (plainEnglish(candidate)) return candidate;
        if (!englishTarget || translation == null || translation.isEmpty()) return "";
        String first = translation.split("\\R", 2)[0].trim();
        return hasControl(first) ? "" : first;
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
