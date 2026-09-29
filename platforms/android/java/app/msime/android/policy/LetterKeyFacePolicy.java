package app.msime.android;

import java.util.Locale;

/** Keeps the visible letter face separate from the case sent to the input engine. */
public final class LetterKeyFacePolicy {
    private LetterKeyFacePolicy() { }

    public static boolean displaysUppercase(boolean chineseMode, boolean localMode,
            boolean shifted) {
        return (chineseMode && !localMode) || (!chineseMode && shifted);
    }

    public static String face(String lowercase, boolean chineseMode, boolean localMode,
            boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "";
        return displaysUppercase(chineseMode, localMode, shifted)
            ? lowercase.toUpperCase(Locale.ROOT) : lowercase;
    }

    public static String accessibilityLabel(String lowercase, boolean chineseMode,
            boolean localMode, boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "字母";
        String uppercase = lowercase.toUpperCase(Locale.ROOT);
        return (!chineseMode && shifted ? "大写 " : "字母 ") + uppercase;
    }
}
