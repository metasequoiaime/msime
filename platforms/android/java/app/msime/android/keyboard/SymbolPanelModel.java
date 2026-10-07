package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

/** Immutable symbol categories shared by the Android full-screen symbol panel. */
public final class SymbolPanelModel {
    public static final int COLUMNS = 5;
    /** 「常用」最多记这么多个，正好六行。 */
    public static final int RECENTS_LIMIT = 30;
    /** 最长的内置条目是「@gmail.com」这类网络后缀；存储里超长的条目当作损坏丢掉。 */
    public static final int MAX_SYMBOL_CODE_POINTS = 32;
    public static final String RECENTS_TITLE = "常用";
    public static final String RECENTS_EMPTY_HINT = "点过的符号会出现在这里";

    public static final class Category {
        private final String title;
        private final List<String> symbols;

        private Category(String title, List<String> symbols) {
            this.title = title;
            this.symbols = List.copyOf(symbols);
        }

        public String title() { return title; }
        public List<String> symbols() { return symbols; }
    }

    // 「常用」不再是写死的一份表，而是用户在面板里实际点过的符号（#5671），所以这里只有固定的分类。原先写在「常用」里的中文标点和全角符号挪到「中文」最前面，免得它们从面板里消失。
    private static final List<Category> FIXED_CATEGORIES = List.of(
        new Category("中文", List.of(
            "，", "。", "？", "！", "、", "；", "：", "…", "—", "·",
            "“", "”", "‘", "’", "（", "）", "《", "》", "【", "】",
            "～", "＆", "＃", "＠", "％", "＋", "－", "＝", "／",
            "〈", "〉", "「", "」", "『", "』", "〔", "〕", "〖", "〗",
            "＜", "＞", "｛", "｝", "［", "］", "︵", "︶", "﹁", "﹂",
            "￥", "〇", "※", "°", "℃", "±", "×", "÷", "≈", "≠",
            "≤", "≥", "√", "∞", "∵", "∴", "→", "←", "↑", "↓",
            "★", "☆", "●", "○", "■", "□", "◆", "◇", "▲", "△")),
        new Category("英文", List.of(
            ",", ".", "?", "!", ";", ":", "'", "\"", "(", ")",
            "[", "]", "{", "}", "<", ">", "/", "\\", "|", "-",
            "_", "+", "=", "*", "&", "^", "%", "$", "#", "@",
            "~", "`", "·", "…", "–", "—", "§", "¶", "†", "‡")),
        new Category("数字", List.of(
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
            "①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩",
            "一", "二", "三", "四", "五", "六", "七", "八", "九", "十",
            "Ⅰ", "Ⅱ", "Ⅲ", "Ⅳ", "Ⅴ", "Ⅵ", "Ⅶ", "Ⅷ", "Ⅸ", "Ⅹ",
            "½", "⅓", "¼", "‰", "′", "″", "㎡", "㎏", "㎝", "№")),
        new Category("网络", List.of(
            "@", "#", "/", "\\", ":", "_", "-", "+", "=", "&",
            "?", "%", "~", "^", "*", "|", "<", ">", "$", "€",
            "http://", "https://", "www.", ".com", ".cn", ".net", ".org", ".io", "@qq.com", "@gmail.com")));

    private SymbolPanelModel() { }

    /** 左列的全部分类：第一个是「常用」，内容就是 `recents`（最近点的在前），后面是固定分类。 */
    public static List<Category> categories(List<String> recents) {
        ArrayList<Category> values = new ArrayList<>(FIXED_CATEGORIES.size() + 1);
        values.add(new Category(RECENTS_TITLE, normalizeRecents(recents)));
        values.addAll(FIXED_CATEGORIES);
        return List.copyOf(values);
    }

    /** 打开面板时先显示哪一类：有使用记录就是「常用」，还没有就是「中文」，不让第一屏是空的。 */
    public static int initialCategory(List<String> recents) {
        return normalizeRecents(recents).isEmpty() ? 1 : 0;
    }

    /** 读回存储的记录：丢掉空串、超长条目和重复项，最多保留 {@link #RECENTS_LIMIT} 个。 */
    public static List<String> normalizeRecents(List<String> stored) {
        LinkedHashSet<String> unique = new LinkedHashSet<>(RECENTS_LIMIT);
        if (stored != null) {
            for (String symbol : stored) {
                if (!validSymbol(symbol)) continue;
                unique.add(symbol);
                if (unique.size() == RECENTS_LIMIT) break;
            }
        }
        return List.copyOf(unique);
    }

    /** 点了 `selected` 之后的记录：它排到最前，原来的位置去掉，超出上限的最旧一个被挤掉。 */
    public static List<String> recordRecent(List<String> stored, String selected) {
        if (!validSymbol(selected)) throw new IllegalArgumentException("Invalid recent symbol");
        ArrayList<String> reordered = new ArrayList<>(RECENTS_LIMIT);
        reordered.add(selected);
        for (String symbol : normalizeRecents(stored)) {
            if (reordered.size() == RECENTS_LIMIT) break;
            if (!selected.equals(symbol)) reordered.add(symbol);
        }
        return List.copyOf(reordered);
    }

    private static boolean validSymbol(String symbol) {
        return symbol != null && !symbol.isEmpty()
            && TextPolicy.withinCodePoints(symbol, MAX_SYMBOL_CODE_POINTS);
    }

    public static boolean closesAfterInsert(boolean locked) { return !locked; }
}
