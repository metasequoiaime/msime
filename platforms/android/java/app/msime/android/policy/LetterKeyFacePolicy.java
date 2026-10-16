package app.msime.android;

/** Keeps the visible letter face separate from the case sent to the input engine. */
public final class LetterKeyFacePolicy {
    private LetterKeyFacePolicy() { }

    /** 中文模式下中文方案（全拼、双拼、五笔、粤拼等）的 26 个字母键面画大写，与搜狗、微信、百度、豆包的拼音 26 键一致，送给 Engine 的仍是小写拼写；与 iOS 的 `updateLetterCaseControls` 和鸿蒙的 `LetterKeyFacePolicy.ts` 同一条规则。日语罗马字照日文键盘的惯例不画大写；中文的本地输入模式（英文补全、Unicode、快捷短语等）按字面输入，也不画大写。其余情况（英文模式、日语罗马字、本地输入模式）平时画小写，只有下一键真的会输入大写时（{@link #typesUppercase}）才画大写。越南语、藏文的字母区分大小写，调用方传入的 chineseMode 对它们为 false，键面按 Shift 画大小写。 */
    public static boolean displaysUppercase(boolean chineseMode, boolean japanese,
            boolean localMode, boolean shifted) {
        return chineseMode && !japanese && !localMode || typesUppercase(chineseMode, localMode, shifted);
    }

    /** 下一键是否真的输入大写：按了 Shift 时 type() 送的是大写，中文模式下也一样（Shift 在辅助码、字母组词里不切英文而是锁大写）。唯一的例外是中文的本地输入模式：那里按字面输入，不当作大写。 */
    static boolean typesUppercase(boolean chineseMode, boolean localMode, boolean shifted) {
        return shifted && !(chineseMode && localMode);
    }

    public static String face(String lowercase, boolean chineseMode, boolean japanese,
            boolean localMode, boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "";
        return displaysUppercase(chineseMode, japanese, localMode, shifted)
            ? TextPolicy.uppercase(lowercase) : TextPolicy.lowercase(lowercase);
    }

    /** 无障碍描述只跟输入的大小写走，不跟键面：下一键真的输入大写时读「大写 N」，其他情况（包括中文模式画成大写的键面）读「字母 N」。 */
    public static String accessibilityLabel(String lowercase, boolean chineseMode,
            boolean localMode, boolean shifted) {
        if (lowercase == null || lowercase.isEmpty()) return "字母";
        String uppercase = TextPolicy.uppercase(lowercase);
        return (typesUppercase(chineseMode, localMode, shifted) ? "大写 " : "字母 ") + uppercase;
    }
}
