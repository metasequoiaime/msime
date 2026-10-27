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
        return translation.split("\\R", 2)[0].trim();
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
