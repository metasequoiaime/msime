package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.function.Predicate;

/** Immutable symbol categories shared by the Android full-screen symbol panel. */
public final class SymbolPanelModel {
    public static final int COLUMNS = 5;
    /** 「常用」最多记这么多个，正好六行。 */
    public static final int RECENTS_LIMIT = 30;
    /** 最长的内置条目是「@protonmail.com」这类邮箱后缀（15 个码位）；存储里超长的条目当作损坏丢掉。 */
    public static final int MAX_SYMBOL_CODE_POINTS = 32;
    /** 不超过这么多码位的条目在五列格里用正常字号；更长的（`http://`、邮箱后缀）缩小字号并单行省略，见 {@link #cellTextSizeSp}。 */
    public static final int SHORT_SYMBOL_CODE_POINTS = 4;
    public static final String RECENTS_TITLE = "常用";
    public static final String RECENTS_EMPTY_HINT = "点过的符号会出现在这里";
    /** Engine 目录（`msime-others.db` 的 `kaomoji_catalog`、`symbol_catalog`）里的两种分类，就是 `msime_client_emoji_catalog_request` 的 `category`。 */
    public static final String KAOMOJI_CATALOG = "kaomoji";
    public static final String SYMBOLS_CATALOG = "symbols";
    /** 目录分类每次向 Engine 要这么多条，滚到底再要下一页。 */
    public static final int CATALOG_PAGE_SIZE = 64;
    /** 一个目录分类最多显示这么多条；随包最大的「字母」有五百多条，颜文字一千多条。 */
    public static final int MAX_CATALOG_ITEMS = 2_048;
    /** 颜文字比单个符号长得多，一行只放两个。 */
    public static final int KAOMOJI_COLUMNS = 2;
    private static final int MAX_KAOMOJI_CODE_POINTS = 96;

    /** 一个分类：写死在这里的（`catalog` 为空，`symbols` 就是全部内容），或者打开时从 Engine 目录分页读的（`symbols` 为空）。 */
    public static final class Category {
        private final String title;
        private final List<String> symbols;
        private final String catalog;
        private final String parent;

        private Category(String title, List<String> symbols) {
            this(title, symbols, "", "");
        }

        private Category(String title, List<String> symbols, String catalog, String parent) {
            this.title = title;
            this.symbols = List.copyOf(symbols);
            this.catalog = catalog;
            this.parent = parent;
        }

        public String title() { return title; }
        public List<String> symbols() { return symbols; }
        /** 空串表示写死的分类；否则是 {@link #KAOMOJI_CATALOG} 或 {@link #SYMBOLS_CATALOG}。 */
        public String catalog() { return catalog; }
        /** 符号目录的上级分类（`symbol_catalog.parent_category`），颜文字和写死的分类为空。 */
        public String parent() { return parent; }
        public boolean fromCatalog() { return !catalog.isEmpty(); }
        public boolean kaomoji() { return KAOMOJI_CATALOG.equals(catalog); }
        public int columns() { return kaomoji() ? KAOMOJI_COLUMNS : COLUMNS; }
        /** 点过的是否记进「常用」：颜文字太长，放进每行五格的「常用」里只剩半截，不记。 */
        public boolean remembers() { return !kaomoji(); }
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
        new Category("网络", network()));

    /**
     * 写死的分类之后，是 Engine 目录里的颜文字和各类符号（#5667「文本符号过少」）。数据随词库 `msime-others.db` 发布，与 iOS 键盘和桌面面板读的是同一份；上级分类的中文名与 iOS 的 `KeyboardEmojiCatalog.symbolParentTitles` 相同，顺序按目录里的顺序。
     */
    private static final List<Category> CATALOG_CATEGORIES = List.of(
        new Category("颜文字", List.of(), KAOMOJI_CATALOG, ""),
        new Category("形状", List.of(), SYMBOLS_CATALOG, "Stars and shapes"),
        new Category("箭头", List.of(), SYMBOLS_CATALOG, "Arrows and lines"),
        new Category("标点", List.of(), SYMBOLS_CATALOG, "Punctuation"),
        new Category("数学", List.of(), SYMBOLS_CATALOG, "Math"),
        new Category("货币", List.of(), SYMBOLS_CATALOG, "Currency"),
        new Category("爱心", List.of(), SYMBOLS_CATALOG, "Hearts"),
        new Category("字母", List.of(), SYMBOLS_CATALOG, "Letters"),
        new Category("游戏", List.of(), SYMBOLS_CATALOG, "Games"),
        new Category("文化", List.of(), SYMBOLS_CATALOG, "Culture"),
        new Category("自然", List.of(), SYMBOLS_CATALOG, "Animals and nature"),
        new Category("人物", List.of(), SYMBOLS_CATALOG, "People and activity"),
        new Category("更多", List.of(), SYMBOLS_CATALOG, "More"));

    private SymbolPanelModel() { }

    /** 「网络」分类：网址和常用符号，后面接全部邮箱后缀（{@link EmailSuffixPolicy#SUFFIXES}，#6147），和邮箱输入框里候选栏给的是同一份。 */
    private static List<String> network() {
        ArrayList<String> values = new ArrayList<>(List.of(
            "@", "#", "/", "\\", ":", "_", "-", "+", "=", "&",
            "?", "%", "~", "^", "*", "|", "<", ">", "$", "€",
            "http://", "https://", "www.", ".com", ".cn", ".net", ".org", ".io"));
        values.addAll(EmailSuffixPolicy.SUFFIXES);
        return values;
    }

    /**
     * 符号格里一个条目的字号（sp）：颜文字 14；不超过 {@link #SHORT_SYMBOL_CODE_POINTS} 个码位的 18；更长的在五列格里放不下，9 个码位以内 13，再长 11，并由面板设成单行、放不下时末尾省略，不折行（#6147）。
     */
    public static float cellTextSizeSp(Category category, String symbol) {
        if (category.kaomoji()) return 14;
        int length = TextPolicy.codePointLength(symbol);
        if (length <= SHORT_SYMBOL_CODE_POINTS) return 18;
        return length <= 9 ? 13 : 11;
    }

    /** 这个条目要不要单行省略：比 {@link #SHORT_SYMBOL_CODE_POINTS} 长的非颜文字条目。 */
    public static boolean singleLineCell(Category category, String symbol) {
        return !category.kaomoji() && symbol != null
            && TextPolicy.codePointLength(symbol) > SHORT_SYMBOL_CODE_POINTS;
    }

    /** 左列的全部分类：第一个是「常用」，内容就是 `recents`（最近点的在前），然后是写死的分类，最后是 Engine 目录里的分类。 */
    public static List<Category> categories(List<String> recents) {
        ArrayList<Category> values = new ArrayList<>(FIXED_CATEGORIES.size() + 1);
        values.add(new Category(RECENTS_TITLE, normalizeRecents(recents)));
        values.addAll(FIXED_CATEGORIES);
        values.addAll(CATALOG_CATEGORIES);
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

    /** 能不能记进「常用」：空串和超过 {@link #MAX_SYMBOL_CODE_POINTS} 个码位的不记。 */
    public static boolean recordable(String symbol) { return validSymbol(symbol); }

    /** Engine 目录返回的一条是否可以显示：非空、不含 NUL、颜文字不超过 96 个码位、符号不超过 32 个。 */
    public static boolean validCatalogText(String text, boolean kaomoji) {
        return text != null && !text.isEmpty() && text.indexOf('\u0000') < 0
            && TextPolicy.withinCodePoints(text, kaomoji ? MAX_KAOMOJI_CODE_POINTS : MAX_SYMBOL_CODE_POINTS);
    }

    /** 一页的游标是否自洽：条数不超过页大小，下一页从本页之后开始、前进不超过页大小，没读完时必须前进。与表情目录的规则相同。 */
    public static boolean validCatalogCursor(int requestedOffset, int count, long nextOffset, boolean complete) {
        return requestedOffset >= 0 && count >= 0 && count <= CATALOG_PAGE_SIZE
            && nextOffset >= requestedOffset && nextOffset <= (long) requestedOffset + CATALOG_PAGE_SIZE
            && (complete || nextOffset > requestedOffset);
    }

    /** 把新一页接在已显示的后面：同一个符号在目录里可能属于好几个小类（`symbol_catalog` 的主键是符号加小类），只显示第一次；总数不超过 {@link #MAX_CATALOG_ITEMS}。 */
    public static List<String> appendCatalogPage(List<String> shown, List<String> page) {
        LinkedHashSet<String> unique = new LinkedHashSet<>(shown);
        for (String text : page) {
            if (unique.size() >= MAX_CATALOG_ITEMS) break;
            unique.add(text);
        }
        return List.copyOf(unique);
    }

    /**
     * 去掉这台设备字体画不出来的目录符号（#6070）：「箭头」里的 ⭠⭡⭢⭣⭤⭥⮂⮃⮐⮑、「爱心」里 Unicode 15 的 🩷🩵🩶 在 Android 11 的系统字体里没有字形，格子是空白，点了上屏的也是一个看不见的字。`drawable` 在设备上是 `Paint.hasGlyph`，与表情面板的过滤相同；它只在整串能排成一个字形时才回答「能画」，所以只用在一条就是一个字形的符号上。颜文字是好几个字形拼成的，`hasGlyph` 对它一律回答「不能」，原样保留。游标照旧按目录扫过的行数前进，这里只影响显示哪些。
     */
    public static List<String> renderableCatalogItems(List<String> items, boolean kaomoji, Predicate<String> drawable) {
        if (kaomoji) return List.copyOf(items);
        ArrayList<String> kept = new ArrayList<>(items.size());
        for (String text : items) if (drawable.test(text)) kept.add(text);
        return List.copyOf(kept);
    }

    /**
     * 「常用」里去掉这台设备画不出来的符号（#6070）：目录过滤上线前点过的空白格（⭠、🩷 等）已经记进了「常用」，而面板一打开就是「常用」，不过滤的话第一屏仍是空白格。「常用」里还有「http://」「@gmail.com」这类多字形的条目，`hasGlyph` 对它们一律回答「不能」，所以只判断一个码位（可带变体选择符）的条目，其余原样保留。
     */
    public static List<String> renderableRecents(List<String> recents, Predicate<String> drawable) {
        ArrayList<String> kept = new ArrayList<>(recents.size());
        for (String text : recents) if (!singleCodePoint(text) || drawable.test(text)) kept.add(text);
        return List.copyOf(kept);
    }

    private static boolean singleCodePoint(String text) {
        int count = 0;
        for (int index = 0; index < text.length(); ) {
            int codePoint = text.codePointAt(index);
            index += Character.charCount(codePoint);
            boolean variationSelector = (codePoint >= 0xFE00 && codePoint <= 0xFE0F)
                || (codePoint >= 0xE0100 && codePoint <= 0xE01EF);
            if (!variationSelector && ++count > 1) return false;
        }
        return count == 1;
    }

    /** 已显示的条数到了上限，就当作读完了，不再要下一页。 */
    public static boolean catalogFull(List<String> shown) { return shown.size() >= MAX_CATALOG_ITEMS; }

    private static boolean validSymbol(String symbol) {
        return symbol != null && !symbol.isEmpty()
            && TextPolicy.withinCodePoints(symbol, MAX_SYMBOL_CODE_POINTS);
    }

    public static boolean closesAfterInsert(boolean locked) { return !locked; }
}
