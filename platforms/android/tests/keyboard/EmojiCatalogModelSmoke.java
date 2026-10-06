import app.msime.android.EmojiCatalogModel;
import java.util.ArrayList;
import java.util.List;

public final class EmojiCatalogModelSmoke {
    public static void main(String[] args) {
        List<String> groups = EmojiCatalogModel.categories().stream()
            .map(EmojiCatalogModel.Category::group).toList();
        check(groups.equals(List.of("Smileys and emotion", "People and body",
            "Animals and nature", "Food and drink", "Travel and places", "Activities",
            "Objects", "Symbols", "Flags")), "Unicode group order");
        List<String> icons = new ArrayList<>(EmojiCatalogModel.categories().stream()
            .map(EmojiCatalogModel.Category::icon).toList());
        icons.add(EmojiCatalogModel.RECENTS.icon());
        check(icons.stream().noneMatch(String::isEmpty) && icons.stream().distinct().count() == icons.size(),
            "every category tab has its own icon");
        check(EmojiCatalogModel.COLUMNS == 8 && EmojiCatalogModel.PAGE_SIZE == 64,
            "eight-column bounded pages");

        EmojiCatalogModel.Item item = new EmojiCatalogModel.Item("🌲", "tree", "fixture");
        EmojiCatalogModel.Page page = EmojiCatalogModel.validatePage(List.of(item), 0, 64, 64, false);
        check(page.nextOffset() == 64 && !page.complete(), "cursor advances by scanned rows");
        EmojiCatalogModel.Page empty = EmojiCatalogModel.validatePage(List.of(), 64, 64, 128, false);
        check(empty.items().isEmpty() && empty.nextOffset() == 128,
            "empty scan page does not stall cursor");
        EmojiCatalogModel.Page tail = EmojiCatalogModel.validatePage(List.of(item), 128, 64, 129, true);
        check(tail.complete(), "short tail completes cursor");

        EmojiCatalogModel.Item missing = new EmojiCatalogModel.Item("🫠", "melting", "fixture");
        EmojiCatalogModel.Page mixed = EmojiCatalogModel.validatePage(
            List.of(item, missing), 0, 64, 64, false);
        EmojiCatalogModel.Page drawn = EmojiCatalogModel.renderable(mixed, text -> !text.equals("🫠"));
        check(drawn.items().equals(List.of(item)) && drawn.nextOffset() == 64 && !drawn.complete(),
            "undrawable entries are dropped without moving the cursor");
        check(EmojiCatalogModel.renderable(mixed, text -> false).items().isEmpty(),
            "fully undrawable page stays a valid empty scan page");

        expectFailure(() -> EmojiCatalogModel.validatePage(List.of(), 0, 64, 0, false));
        expectFailure(() -> EmojiCatalogModel.validatePage(List.of(), 64, 64, 63, true));
        expectFailure(() -> EmojiCatalogModel.validatePage(List.of(), 0, 64, 65, false));
        ArrayList<EmojiCatalogModel.Item> oversized = new ArrayList<>();
        for (int index = 0; index < 65; index++) oversized.add(item);
        expectFailure(() -> EmojiCatalogModel.validatePage(oversized, 0, 64, 64, false));
        expectFailure(() -> new EmojiCatalogModel.Item("", "", "fixture"));

        ArrayList<String> selections = new ArrayList<>();
        for (int index = 0; index < 30; index++) selections = new ArrayList<>(
            EmojiCatalogModel.recordRecent(selections, "fixture-" + index));
        check(selections.size() == 24 && selections.get(0).equals("fixture-29")
            && selections.get(23).equals("fixture-6"), "recent limit and order");
        selections = new ArrayList<>(EmojiCatalogModel.recordRecent(selections, "fixture-10"));
        check(selections.size() == 24 && selections.get(0).equals("fixture-10")
            && selections.stream().filter("fixture-10"::equals).count() == 1,
            "recent selection moves without duplication");
        check(EmojiCatalogModel.normalizeRecents(List.of("🌲", "🌲", "", "🌊"))
            .equals(List.of("🌲", "🌊")), "persisted recents are normalized");
        System.out.println("Emoji catalog model: group order, cursor bounds, glyph filtering and recents passed");
    }

    private static void expectFailure(Runnable action) {
        try { action.run(); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("Invalid emoji catalog value accepted");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
