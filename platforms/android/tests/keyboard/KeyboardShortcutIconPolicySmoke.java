import app.msime.android.KeyboardShortcutIconPolicy;
import app.msime.android.KeyboardShortcutIconPolicy.Icon;

public final class KeyboardShortcutIconPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(KeyboardShortcutIconPolicy.forLabel("设置") == Icon.SETTINGS,
            "settings glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("回复") == Icon.REPLY,
            "reply glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("☺") == Icon.EMOJI,
            "emoji glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("语音") == Icon.VOICE,
            "voice glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("皮肤") == Icon.SKIN,
            "skin glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("收起") == Icon.DISMISS,
            "dismiss glyph mapping");
        boolean rejected = false;
        try { KeyboardShortcutIconPolicy.forLabel("未知"); }
        catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "unknown shortcut labels rejected");
        System.out.println("Android shortcut bar: Apple glyph mappings passed");
    }
}
