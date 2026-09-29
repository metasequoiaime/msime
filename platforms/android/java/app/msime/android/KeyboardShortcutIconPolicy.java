package app.msime.android;

/** Fixed Apple-style visual mapping for the Android shortcut strip. */
public final class KeyboardShortcutIconPolicy {
    public enum Icon { SETTINGS, REPLY, EMOJI, VOICE, SKIN, DISMISS, GLOBE }

    private KeyboardShortcutIconPolicy() {}

    public static Icon forLabel(String label) {
        return switch (label) {
            case "设置" -> Icon.SETTINGS;
            case "回复" -> Icon.REPLY;
            case "☺" -> Icon.EMOJI;
            case "语音" -> Icon.VOICE;
            case "皮肤" -> Icon.SKIN;
            case "收起" -> Icon.DISMISS;
            case "切换" -> Icon.GLOBE;
            default -> throw new IllegalArgumentException("No shortcut icon for: " + label);
        };
    }
}
