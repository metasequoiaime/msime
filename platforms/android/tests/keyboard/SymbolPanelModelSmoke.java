import app.msime.android.SymbolPanelModel;

import java.util.ArrayList;
import java.util.List;

public final class SymbolPanelModelSmoke {
    public static void main(String[] args) {
        List<SymbolPanelModel.Category> empty = SymbolPanelModel.categories(List.of());
        check(empty.size() == 5 + 13, "five fixed categories, kaomoji and twelve symbol parents: " + empty.size());
        check(empty.get(0).title().equals("常用") && empty.get(0).symbols().isEmpty(),
            "常用 starts empty instead of a hard-coded list");
        check(empty.get(4).symbols().get(20).equals("http://"), "network shortcuts");
        // 原先只在「常用」里的中文标点挪到了「中文」，没有从面板里消失。
        for (String mark : List.of("，", "。", "？", "！", "、", "“", "”", "（", "）", "《", "》", "【", "】", "＠", "／"))
            check(empty.get(1).symbols().contains(mark), "中文 keeps " + mark);
        for (SymbolPanelModel.Category category : empty)
            check(category.symbols().size() == new java.util.HashSet<>(category.symbols()).size(),
                category.title() + " has no duplicates");
        catalogCategories(empty);

        check(SymbolPanelModel.initialCategory(List.of()) == 1, "no history opens on 中文");
        check(SymbolPanelModel.initialCategory(List.of("＞")) == 0, "history opens on 常用");

        // 点过的符号排到最前，重复点不会出现两次。
        List<String> recents = SymbolPanelModel.recordRecent(List.of(), "＞");
        recents = SymbolPanelModel.recordRecent(recents, "。");
        recents = SymbolPanelModel.recordRecent(recents, "＞");
        check(recents.equals(List.of("＞", "。")), "most recent first, deduplicated: " + recents);
        check(SymbolPanelModel.categories(recents).get(0).symbols().equals(recents), "常用 shows the history");

        // 上限 30，最旧的被挤掉。
        List<String> full = new ArrayList<>();
        for (int index = 0; index < SymbolPanelModel.RECENTS_LIMIT; index++) full.add("s" + index);
        List<String> pushed = SymbolPanelModel.recordRecent(full, "new");
        check(pushed.size() == SymbolPanelModel.RECENTS_LIMIT, "limit");
        check(pushed.get(0).equals("new") && !pushed.contains("s29") && pushed.contains("s28"), "oldest dropped");

        // 存储里的坏数据：空串、null、超长、重复都被丢掉。
        List<String> stored = new ArrayList<>();
        stored.add(null);
        stored.add("");
        stored.add("x".repeat(SymbolPanelModel.MAX_SYMBOL_CODE_POINTS + 1));
        stored.add("@gmail.com");
        stored.add("@gmail.com");
        check(SymbolPanelModel.normalizeRecents(stored).equals(List.of("@gmail.com")), "normalize stored");
        try {
            SymbolPanelModel.recordRecent(List.of(), "");
            throw new AssertionError("empty symbol recorded");
        } catch (IllegalArgumentException expected) {
            // 空串不是一个符号。
        }

        check(SymbolPanelModel.closesAfterInsert(false) && !SymbolPanelModel.closesAfterInsert(true), "lock");
        System.out.println("SymbolPanelModelSmoke: PASS");
    }

    /** #5667：颜文字和 Engine 符号目录接在写死的分类后面。 */
    private static void catalogCategories(List<SymbolPanelModel.Category> categories) {
        for (int index = 0; index < 5; index++) {
            SymbolPanelModel.Category fixed = categories.get(index);
            check(!fixed.fromCatalog() && fixed.columns() == SymbolPanelModel.COLUMNS && fixed.remembers(),
                fixed.title() + " is a fixed five-column category");
        }
        SymbolPanelModel.Category kaomoji = categories.get(5);
        check(kaomoji.title().equals("颜文字") && kaomoji.kaomoji() && kaomoji.fromCatalog()
            && kaomoji.symbols().isEmpty(), "颜文字 comes from the Engine catalog");
        check(kaomoji.columns() == SymbolPanelModel.KAOMOJI_COLUMNS && !kaomoji.remembers(),
            "kaomoji are two per row and stay out of 常用");
        java.util.HashSet<String> parents = new java.util.HashSet<>();
        for (SymbolPanelModel.Category category : categories.subList(6, categories.size())) {
            check(category.catalog().equals(SymbolPanelModel.SYMBOLS_CATALOG) && !category.parent().isEmpty()
                && category.columns() == SymbolPanelModel.COLUMNS && category.remembers(),
                category.title() + " reads one symbol parent");
            check(parents.add(category.parent()), category.parent() + " listed once");
        }
        check(parents.contains("Arrows and lines") && parents.contains("Letters") && parents.contains("Math"),
            "the catalog's parents are reachable");

        // 目录内容的校验：颜文字允许更长，符号与「常用」同样最多 32 个码位，NUL 和空串一律不要。
        check(SymbolPanelModel.validCatalogText("→", false), "symbol");
        check(SymbolPanelModel.validCatalogText("(ノ°▽°)ノ︵┻━┻" + "~".repeat(40), true), "a long kaomoji");
        check(!SymbolPanelModel.validCatalogText("~".repeat(SymbolPanelModel.MAX_SYMBOL_CODE_POINTS + 1), false),
            "an overlong symbol");
        check(!SymbolPanelModel.validCatalogText("", true) && !SymbolPanelModel.validCatalogText(null, false)
            && !SymbolPanelModel.validCatalogText("a\u0000b", true), "empty or NUL");
        check(SymbolPanelModel.recordable("→") && !SymbolPanelModel.recordable("~".repeat(40)), "recordable");

        // 游标规则与表情目录相同。
        int page = SymbolPanelModel.CATALOG_PAGE_SIZE;
        check(SymbolPanelModel.validCatalogCursor(0, page, page, false), "a full page");
        check(SymbolPanelModel.validCatalogCursor(page, 3, page + 3, true), "the last page");
        check(!SymbolPanelModel.validCatalogCursor(0, page + 1, page + 1, false), "too many rows");
        check(!SymbolPanelModel.validCatalogCursor(page, 0, page, false), "an incomplete page must advance");
        check(!SymbolPanelModel.validCatalogCursor(page, 1, page - 1, false), "a cursor never goes back");
        check(!SymbolPanelModel.validCatalogCursor(0, 1, page + 1, false), "a cursor never skips");

        // 同一个符号在目录里属于几个小类时只显示一次，总数有上限。
        check(SymbolPanelModel.appendCatalogPage(List.of("→", "←"), List.of("←", "↑")).equals(List.of("→", "←", "↑")),
            "pages are deduplicated");
        List<String> many = new ArrayList<>();
        for (int index = 0; index < SymbolPanelModel.MAX_CATALOG_ITEMS + 10; index++) many.add("s" + index);
        List<String> capped = SymbolPanelModel.appendCatalogPage(List.of(), many);
        check(capped.size() == SymbolPanelModel.MAX_CATALOG_ITEMS && SymbolPanelModel.catalogFull(capped), "capped");
        check(!SymbolPanelModel.catalogFull(List.of("→")), "not full");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
