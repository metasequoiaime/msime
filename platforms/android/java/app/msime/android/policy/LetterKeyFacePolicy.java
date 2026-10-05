package app.msime.android;

import java.util.Locale;

/** Keeps the visible letter face separate from the case sent to the input engine. */
public final class LetterKeyFacePolicy {
    private LetterKeyFacePolicy() { }

    /** 新设计里中文模式的 26 个字母键一律画小写（与设计稿 in_default 一致）；只有英文模式下按了 Shift 才画大写。本地输入同样小写。 */
    public static boolean displaysUppercase(boolean chineseMode, boolean localMode,
            boolean shifted) {
        return !chineseMode && shifted;
    }

    public static String face(String lowercase, boolean chineseMode, boolean localMode,
            boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "";
        return displaysUppercase(chineseMode, localMode, shifted)
            ? lowercase.toUpperCase(Locale.ROOT) : lowercase.toLowerCase(Locale.ROOT);
    }

    /** 无障碍描述格式不变：英文模式按了 Shift 读「大写 N」，其他情况读「字母 N」。 */
    public static String accessibilityLabel(String lowercase, boolean chineseMode,
            boolean localMode, boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "字母";
        String uppercase = lowercase.toUpperCase(Locale.ROOT);
        return (!chineseMode && shifted ? "大写 " : "字母 ") + uppercase;
    }
}
