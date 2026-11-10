package app.msime.android;

/** 关闭拼写编辑后去掉入口独占的行；必要的读音、模式和编辑动作继续保留。 */
public final class SpellingEditorPolicy {
    private SpellingEditorPolicy() { }

    public static boolean reserveHeader(boolean enabled, int scheme, boolean keyboardSpelling,
            boolean idle, CharSequence spelling, CharSequence status, boolean actions) {
        boolean spellingScheme = scheme == InputSchemeTraits.QUANPIN
            || scheme == InputSchemeTraits.SHUANGPIN || scheme == InputSchemeTraits.WUBI
            || scheme == InputSchemeTraits.CANTONESE;
        if (enabled || !spellingScheme || keyboardSpelling) return true;
        return hasText(status) || !idle && (hasText(spelling) || actions);
    }

    private static boolean hasText(CharSequence text) {
        return text != null && !text.toString().trim().isEmpty();
    }
}
