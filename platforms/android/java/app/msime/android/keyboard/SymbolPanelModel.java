package app.msime.android;

import java.util.List;

/** Immutable symbol categories shared by the Android full-screen symbol panel. */
public final class SymbolPanelModel {
    public static final int COLUMNS = 5;

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

    private static final List<Category> CATEGORIES = List.of(
        new Category("常用", List.of(
            "，", "。", "？", "！", "、", "；", "：", "…", "—", "·",
            "“", "”", "‘", "’", "（", "）", "《", "》", "【", "】",
            "～", "￥", "＆", "＃", "＠", "％", "＋", "－", "＝", "／")),
        new Category("中文", List.of(
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

    public static List<Category> categories() { return CATEGORIES; }
    public static boolean closesAfterInsert(boolean locked) { return !locked; }
}
