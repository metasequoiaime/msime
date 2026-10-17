package app.msime.android;

import java.util.List;

/** 符号分类排列的容错、稳定身份和边界移动回归。 */
public final class SymbolCategoryOrderPolicySmoke {
    public static void main(String[] args) {
        List<SymbolCategoryOrderPolicy.Item> defaults = SymbolCategoryOrderPolicy.DEFAULT_ORDER;
        check(SymbolCategoryOrderPolicy.ordered(null).equals(defaults), "missing order defaults");
        check(SymbolCategoryOrderPolicy.ordered("").equals(defaults), "empty order defaults");
        List<SymbolCategoryOrderPolicy.Item> repaired = SymbolCategoryOrderPolicy.ordered("unknown,math,math");
        check(repaired.get(0) == SymbolCategoryOrderPolicy.Item.MATH && repaired.size() == defaults.size(),
            "unknown and duplicates are repaired");
        List<SymbolCategoryOrderPolicy.Item> custom = SymbolCategoryOrderPolicy.ordered(
            "math,recents,network,math,unknown");
        check(custom.get(0) == SymbolCategoryOrderPolicy.Item.MATH
            && custom.get(1) == SymbolCategoryOrderPolicy.Item.RECENTS
            && custom.get(2) == SymbolCategoryOrderPolicy.Item.NETWORK,
            "known IDs keep the requested prefix");
        check(custom.size() == defaults.size() && custom.containsAll(defaults),
            "missing categories append in default order");
        check(SymbolCategoryOrderPolicy.ordered(SymbolCategoryOrderPolicy.format(custom)).equals(custom),
            "stable IDs round trip");
        check(SymbolCategoryOrderPolicy.ordered(" math , network ").get(0)
            == SymbolCategoryOrderPolicy.Item.MATH, "whitespace accepted");
        check(SymbolCategoryOrderPolicy.normalize("x".repeat(513)) == null, "oversized writes rejected");
        check(SymbolCategoryOrderPolicy.ordered("x".repeat(513)).equals(defaults), "oversized reads default");
        check(SymbolCategoryOrderPolicy.normalize(null) == null, "null writes rejected");
        List<SymbolCategoryOrderPolicy.Item> moved = SymbolCategoryOrderPolicy.move(
            defaults, SymbolCategoryOrderPolicy.Item.MATH, -1);
        check(moved.get(9) == SymbolCategoryOrderPolicy.Item.MATH
            && moved.get(10) == SymbolCategoryOrderPolicy.Item.PUNCTUATION,
            "move preserves neighboring identities");
        check(defaults.get(10) == SymbolCategoryOrderPolicy.Item.MATH, "move leaves source intact");
        boolean immutable = false;
        try { moved.clear(); } catch (UnsupportedOperationException expected) { immutable = true; }
        check(immutable, "returned order is immutable");
        check(AndroidLocalSettings.defaults().text(AndroidLocalSettings.SYMBOL_CATEGORY_ORDER)
            .equals(SymbolCategoryOrderPolicy.format(defaults)), "registered default matches visible panel");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.SYMBOL_CATEGORY_ORDER).synced,
            "order stays local");

        List<SymbolPanelModel.Category> reordered = SymbolPanelModel.categories(List.of(), custom);
        check(reordered.get(0).id().equals("math") && reordered.get(1).id().equals("recents"),
            "model follows the custom order");
        check(SymbolPanelModel.initialCategory(List.of(), custom) == 1,
            "initial category locates 常用 by stable ID");
        check(reordered.get(reordered.size() - 1).id().equals("more"),
            "all categories remain reachable");
        System.out.println("SymbolCategoryOrderPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
