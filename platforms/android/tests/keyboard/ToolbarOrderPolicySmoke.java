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
            ToolbarOrderPolicy.Item.SCHEME, ToolbarOrderPolicy.Item.FLOATING)), "missing items append in default order");
        check(ToolbarOrderPolicy.ordered(ToolbarOrderPolicy.format(custom)).equals(custom), "stable IDs round trip");
        check(ToolbarOrderPolicy.ordered(" skin , clipboard ").get(0) == ToolbarOrderPolicy.Item.SKIN, "whitespace accepted");
        check(ToolbarOrderPolicy.normalize("x".repeat(513)) == null, "oversized writes rejected");
        check(ToolbarOrderPolicy.ordered("x".repeat(513)).equals(defaults), "oversized reads default");
        check(ToolbarOrderPolicy.normalize(null) == null, "null writes rejected");
        check(ToolbarOrderPolicy.move(custom, custom.get(0), -1).equals(custom), "first stays inside bounds");
        check(ToolbarOrderPolicy.move(custom, custom.get(6), 1).equals(custom), "last stays inside bounds");
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
