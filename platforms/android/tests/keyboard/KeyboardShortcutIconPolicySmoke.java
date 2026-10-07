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
        check(KeyboardShortcutIconPolicy.forLabel("表情") == Icon.EMOJI,
            "emoji label mapping");
        check(KeyboardShortcutIconPolicy.forLabel("常用语") == Icon.PHRASE,
            "phrase glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("剪贴板") == Icon.CLIPBOARD,
            "clipboard glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("输入方式") == Icon.SCHEME,
            "scheme glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("语音") == Icon.VOICE,
            "voice glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("皮肤") == Icon.SKIN,
            "skin glyph mapping");
        check(KeyboardShortcutIconPolicy.forLabel("收起") == Icon.DISMISS,
            "dismiss glyph mapping");
        for (Icon icon : new Icon[] {Icon.EMOJI, Icon.PHRASE, Icon.CLIPBOARD, Icon.SKIN,
                Icon.SCHEME, Icon.DISMISS}) {
            check(KeyboardShortcutIconPolicy.materialGlyph(icon), "material toolbar glyph " + icon);
        }
        check(!KeyboardShortcutIconPolicy.materialGlyph(Icon.BOOKMARK), "bookmark keeps stroke glyph");
        boolean rejected = false;
        try { KeyboardShortcutIconPolicy.forLabel("未知"); }
        catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "unknown shortcut labels rejected");
        System.out.println("Android shortcut bar: glyph mappings passed");
    }
}
