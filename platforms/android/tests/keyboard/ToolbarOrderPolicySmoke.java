package app.msime.android;

import java.util.List;

/** 排列容错、隐藏后保留身份和边界移动的纯规则回归。 */
public final class ToolbarOrderPolicySmoke {
    public static void main(String[] args) {
        List<ToolbarOrderPolicy.Item> defaults = ToolbarOrderPolicy.DEFAULT_ORDER;
        check(ToolbarOrderPolicy.ordered(null).equals(defaults), "missing order defaults");
        check(ToolbarOrderPolicy.ordered("").equals(defaults), "empty order defaults");
        check(ToolbarOrderPolicy.ordered("unknown,emoji,emoji").equals(defaults), "unknown and duplicates skipped");
        List<ToolbarOrderPolicy.Item> custom = ToolbarOrderPolicy.ordered("text_edit,skin,emoji,skin,unknown");
        check(custom.equals(List.of(ToolbarOrderPolicy.Item.TEXT_EDIT, ToolbarOrderPolicy.Item.SKIN,
            ToolbarOrderPolicy.Item.EMOJI, ToolbarOrderPolicy.Item.PHRASE, ToolbarOrderPolicy.Item.CLIPBOARD,
            ToolbarOrderPolicy.Item.SCHEME, ToolbarOrderPolicy.Item.FLOATING,
            ToolbarOrderPolicy.Item.UNDO, ToolbarOrderPolicy.Item.REDO, ToolbarOrderPolicy.Item.VOICE, ToolbarOrderPolicy.Item.RANDOM_DIGITS)),
            "missing items append in default order");
        // 新增按钮按引入顺序补在末尾，已有工具保持原顺序和开关归属。
        check(defaults.get(defaults.size() - 1) == ToolbarOrderPolicy.Item.RANDOM_DIGITS, "新增随机数字补在末尾");
        check(!AndroidLocalSettings.defaults().bool(AndroidLocalSettings.TOOLBAR_RANDOM_DIGITS), "随机数字入口默认关闭");
        check(ToolbarOrderPolicy.Item.VOICE.id.equals("voice") && ToolbarOrderPolicy.Item.VOICE.label.equals("语音")
            && ToolbarOrderPolicy.Item.VOICE.preferenceKey.equals("touch_voice_shortcut"), "voice identity and switch");
        String legacy = "redo,undo,text_edit,floating,scheme,skin,clipboard,phrase,emoji";
        List<ToolbarOrderPolicy.Item> upgraded = ToolbarOrderPolicy.ordered(legacy);
        check(ToolbarOrderPolicy.format(upgraded).equals(legacy + ",voice,random_digits"), "a stored order from before voice keeps its order and gains voice");
        check(ToolbarOrderPolicy.normalize(legacy).equals(legacy + ",voice,random_digits"), "normalizing a pre-voice order appends voice");
        check(ToolbarOrderPolicy.ordered("voice,emoji,voice").get(0) == ToolbarOrderPolicy.Item.VOICE
            && ToolbarOrderPolicy.ordered("voice,emoji,voice").size() == defaults.size(), "voice can lead and is not duplicated");
        check(ToolbarOrderPolicy.ordered(ToolbarOrderPolicy.format(custom)).equals(custom), "stable IDs round trip");
        check(ToolbarOrderPolicy.ordered(" skin , clipboard ").get(0) == ToolbarOrderPolicy.Item.SKIN, "whitespace accepted");
        check(ToolbarOrderPolicy.normalize("x".repeat(513)) == null, "oversized writes rejected");
        check(ToolbarOrderPolicy.ordered("x".repeat(513)).equals(defaults), "oversized reads default");
        check(ToolbarOrderPolicy.normalize(null) == null, "null writes rejected");
        check(ToolbarOrderPolicy.move(custom, custom.get(0), -1).equals(custom), "first stays inside bounds");
        check(ToolbarOrderPolicy.move(custom, custom.get(custom.size() - 1), 1).equals(custom), "last stays inside bounds");
        List<ToolbarOrderPolicy.Item> moved = ToolbarOrderPolicy.move(defaults, ToolbarOrderPolicy.Item.SKIN, -1);
        check(moved.get(2) == ToolbarOrderPolicy.Item.SKIN && moved.get(3) == ToolbarOrderPolicy.Item.CLIPBOARD,
            "move preserves neighboring identities");
        check(defaults.get(2) == ToolbarOrderPolicy.Item.CLIPBOARD, "move leaves source intact");
        boolean immutable = false;
        try { moved.clear(); } catch (UnsupportedOperationException expected) { immutable = true; }
        check(immutable, "returned order is immutable");
        check(AndroidLocalSettings.defaults().text(AndroidLocalSettings.TOOLBAR_ORDER)
            .equals(ToolbarOrderPolicy.format(defaults)), "registered default matches visible toolbar");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.TOOLBAR_ORDER).synced, "order stays local");
        check(AndroidLocalSettings.spec(AndroidLocalSettings.TOOLBAR_ORDER).accept(42) == null, "wrong type rejected");
        System.out.println("ToolbarOrderPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
