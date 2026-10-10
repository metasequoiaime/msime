package app.msime.android;

/** Fixed visual mapping for the Android shortcut strip. */
public final class KeyboardShortcutIconPolicy {
    /**
     * `BOOKMARK` 不在快捷栏上，是回复面板里的模板入口，所以 {@link #forLabel} 不映射它。
     *
     * <p>`PHRASE`、`CLIPBOARD`、`SCHEME` 是新设计工具栏上的常用语、剪贴板、输入方式；它们与 `EMOJI`、`SKIN`、`DISMISS` 一起由 {@link KeyboardShortcutButton} 按设计的 Material 实心图标绘制。
     */
    public enum Icon { SETTINGS, REPLY, EMOJI, VOICE, SKIN, DISMISS, GLOBE, BOOKMARK, PHRASE, CLIPBOARD, SCHEME, FLOATING, TEXT_EDIT }

    private KeyboardShortcutIconPolicy() {}

    public static Icon forLabel(String label) {
        return switch (label) {
            case "设置" -> Icon.SETTINGS;
            case "回复" -> Icon.REPLY;
            case "☺", "表情" -> Icon.EMOJI;
            case "语音" -> Icon.VOICE;
            case "皮肤" -> Icon.SKIN;
            case "收起" -> Icon.DISMISS;
            case "切换" -> Icon.GLOBE;
            case "常用语" -> Icon.PHRASE;
            case "剪贴板" -> Icon.CLIPBOARD;
            case "输入方式" -> Icon.SCHEME;
            case "浮动键盘" -> Icon.FLOATING;
            case "文本编辑" -> Icon.TEXT_EDIT;
            default -> throw new IllegalArgumentException("No shortcut icon for: " + label);
        };
    }

    /** 这个图标是否按设计的 Material 实心图标（24 dp）绘制；为假的沿用旧的描边画法。 */
    public static boolean materialGlyph(Icon icon) {
        return switch (icon) {
            case EMOJI, PHRASE, CLIPBOARD, SKIN, SCHEME, FLOATING, TEXT_EDIT, DISMISS -> true;
            case SETTINGS, REPLY, VOICE, GLOBE, BOOKMARK -> false;
        };
    }
}
