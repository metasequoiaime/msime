package app.msime.android;

/** 关闭拼写编辑后去掉入口独占的行；必要的读音、模式和编辑动作继续保留。 */
public final class SpellingEditorPolicy {
    private SpellingEditorPolicy() { }

    /** 只有读音确实可见时才保留编辑入口；空白入口不能再接收误触。 */
    public static boolean entryVisible(boolean enabled, boolean caretEditable, CharSequence title) {
        return enabled && caretEditable && hasText(title);
    }

    /**
     * 屏幕上这副键盘是否从空闲起就预留读音行：只有 14/18 键，它们的拼音条长在读音行上，打字时这一行一定出现，空闲时也留着，键盘不跳。九键和 26 键一样，拼写写进输入框，读音行只按内容出现（{@link #reserveHeader}）。英文键盘屏幕上是 26 键，所以不预留，从哪副中文键盘切来都一样；中文和其他语言键盘之间的高度规则另行处理。
     */
    public static boolean keyboardReadingRow(int displayedLayout) {
        return KeyboardLayout.drawsLetterGrid(displayedLayout);
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
