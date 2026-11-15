package app.msime.android;

/** 关闭拼写编辑后去掉入口独占的行；必要的读音、模式和编辑动作继续保留。 */
public final class SpellingEditorPolicy {
    private SpellingEditorPolicy() { }

    /** 只有读音确实可见时才保留编辑入口；空白入口不能再接收误触。 */
    public static boolean entryVisible(boolean enabled, boolean caretEditable, CharSequence title) {
        return enabled && caretEditable && hasText(title);
    }

    public static boolean reserveHeader(boolean enabled, int scheme, boolean keyboardSpelling,
            boolean idle, CharSequence spelling, CharSequence status, boolean actions) {
        boolean spellingScheme = scheme == InputSchemeTraits.QUANPIN
            || scheme == InputSchemeTraits.SHUANGPIN || scheme == InputSchemeTraits.WUBI
            || scheme == InputSchemeTraits.CANTONESE;
        if (!spellingScheme || keyboardSpelling) return true;
        return hasText(status) || !idle && (hasText(spelling) || actions);
    }

    private static boolean hasText(CharSequence text) {
        return text != null && !text.toString().trim().isEmpty();
    }
}
