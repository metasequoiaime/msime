package app.msime.android;

/** Keeps the visible letter face separate from the case sent to the input engine. */
public final class LetterKeyFacePolicy {
    private LetterKeyFacePolicy() { }

    /** 新设计里 26 个字母键平时画小写（与设计稿 in_default 一致）；按了 Shift 时 type() 送的是大写，键面也跟着画大写，中文模式下也一样（Shift 在辅助码、字母组词里不切英文而是锁大写）。唯一的例外是中文的本地输入模式：那里键面固定画小写。 */
    public static boolean displaysUppercase(boolean chineseMode, boolean localMode,
            boolean shifted) {
        return shifted && !(chineseMode && localMode);
    }

    public static String face(String lowercase, boolean chineseMode, boolean localMode,
            boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "";
        return displaysUppercase(chineseMode, localMode, shifted)
            ? TextPolicy.uppercase(lowercase) : TextPolicy.lowercase(lowercase);
    }

    /** 无障碍描述跟键面一致：键面画大写时读「大写 N」，其他情况读「字母 N」。 */
    public static String accessibilityLabel(String lowercase, boolean chineseMode,
            boolean localMode, boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "字母";
        String uppercase = TextPolicy.uppercase(lowercase);
        return (displaysUppercase(chineseMode, localMode, shifted) ? "大写 " : "字母 ") + uppercase;
    }
}
