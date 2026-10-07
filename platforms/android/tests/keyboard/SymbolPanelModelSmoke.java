import app.msime.android.SymbolPanelModel;

import java.util.ArrayList;
import java.util.List;

public final class SymbolPanelModelSmoke {
    public static void main(String[] args) {
        List<SymbolPanelModel.Category> empty = SymbolPanelModel.categories(List.of());
        check(empty.size() == 5, "categories");
        check(empty.get(0).title().equals("常用") && empty.get(0).symbols().isEmpty(),
            "常用 starts empty instead of a hard-coded list");
        check(empty.get(4).symbols().get(20).equals("http://"), "network shortcuts");
        // 原先只在「常用」里的中文标点挪到了「中文」，没有从面板里消失。
        for (String mark : List.of("，", "。", "？", "！", "、", "“", "”", "（", "）", "《", "》", "【", "】", "＠", "／"))
            check(empty.get(1).symbols().contains(mark), "中文 keeps " + mark);
        for (SymbolPanelModel.Category category : empty)
            check(category.symbols().size() == new java.util.HashSet<>(category.symbols()).size(),
                category.title() + " has no duplicates");

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

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
