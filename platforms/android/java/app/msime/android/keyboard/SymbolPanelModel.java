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
    /** 不超过这么多码位的条目在五列格里用正常字号；更长的（`http://`、邮箱后缀）缩小字号，并在固定分类里按文字宽度跨列，见 {@link #cellColumnSpan}。 */
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

    /** 一个分类：写死在这里的（`catalog` 为空，`symbols` 就是全部内容），打开时从 Engine 目录分页读的（`symbols` 为空），或者符号集插件的（`symbols` 是插件的条目，`pluginKaomoji` 为真时按颜文字排）。 */
    public static final class Category {
        private final String title;
        private final String id;
        private final List<String> symbols;
        private final String catalog;
        private final String parent;
        private final boolean pluginKaomoji;

        private Category(String id, String title, List<String> symbols) {
            this(id, title, symbols, "", "");
        }

        private Category(String id, String title, List<String> symbols, String catalog, String parent) {
            this(id, title, symbols, catalog, parent, false);
        }

        private Category(String id, String title, List<String> symbols, String catalog, String parent,
                boolean pluginKaomoji) {
            this.id = id;
            this.title = title;
            this.symbols = List.copyOf(symbols);
            this.catalog = catalog;
            this.parent = parent;
            this.pluginKaomoji = pluginKaomoji;
        }

        public String id() { return id; }
        public String title() { return title; }
        public List<String> symbols() { return symbols; }
        /** 空串表示写死的分类；否则是 {@link #KAOMOJI_CATALOG} 或 {@link #SYMBOLS_CATALOG}。 */
        public String catalog() { return catalog; }
        /** 符号目录的上级分类（`symbol_catalog.parent_category`），颜文字和写死的分类为空。 */
        public String parent() { return parent; }
        public boolean fromCatalog() { return !catalog.isEmpty(); }
        /** 颜文字：目录里的颜文字，或符号集插件放在颜文字页的一组。 */
        public boolean kaomoji() { return KAOMOJI_CATALOG.equals(catalog) || pluginKaomoji; }
        /** 符号集插件的分类（{@link #pluginCategories}）。 */
        public boolean plugin() { return id.startsWith(PLUGIN_SYMBOLS_PREFIX) || id.startsWith(PLUGIN_KAOMOJI_PREFIX); }
        public int columns() { return kaomoji() ? KAOMOJI_COLUMNS : COLUMNS; }
        /** 点过的是否记进「常用」：颜文字太长，放进每行五格的「常用」里只剩半截，不记。 */
        public boolean remembers() { return !kaomoji(); }

        @Override public boolean equals(Object other) {
            return other instanceof Category that && id.equals(that.id) && title.equals(that.title)
                && symbols.equals(that.symbols) && catalog.equals(that.catalog) && parent.equals(that.parent)
                && pluginKaomoji == that.pluginKaomoji;
        }

        @Override public int hashCode() {
            return java.util.Objects.hash(id, title, symbols, catalog, parent, pluginKaomoji);
        }
    }

    /** 符号集插件分类的 id 前缀：符号页每个插件一个分类，颜文字每组一个分类。不会和内置分类撞名，也不进「符号分类顺序」。 */
    public static final String PLUGIN_SYMBOLS_PREFIX = "plugin-symbols:";
    public static final String PLUGIN_KAOMOJI_PREFIX = "plugin-kaomoji:";
    /** 一组最多接收的条目数，与插件清单的上限（`symbol_set` 的每组 512 项）一致。 */
    public static final int MAX_PLUGIN_GROUP_ITEMS = 512;
    /** 一个条目的 UTF-16 长度上限，与插件清单的上限一致。 */
    public static final int MAX_PLUGIN_ITEM_UNITS = 64;
    /** 插件名和组标题的 UTF-16 长度上限：清单限制组标题 48 字节、插件名 80 字节，这里留足余量，只挡明显坏掉的回答。 */
    private static final int MAX_PLUGIN_NAME_UNITS = 128;

    /**
     * `msime_client_emoji_catalog_request` 在 `list_plugin_symbol_groups` 下回传的一组（字段名与接口一致）：哪个包、包名、放在哪一页（`symbols` 或 `kaomoji`）、组标题和条目。关键词只给搜索用，这个面板没有搜索，不读。
     */
    public record PluginGroup(String pack, String packName, String tab, String title, List<String> items) {}

    /**
     * 符号集插件的分类，与鸿蒙 2in1 面板（`PluginSymbolGroupPolicy`）和 Tauri 面板（`append_plugin_symbol_groups`）同样的排法：`symbols` 组以插件名为分类，每个插件一个，内容是它全部 `symbols` 组按清单顺序首尾相接；`kaomoji` 组逐组成为一个分类，标题是组标题，按颜文字排。坏组跳过而不是整份作废：空名字、未知的页、没有可用条目的组都不出现，超长的条目去掉，一组最多 {@link #MAX_PLUGIN_GROUP_ITEMS} 项。不跨插件、也不与内置分类去重。
     */
    public static List<Category> pluginCategories(List<PluginGroup> groups) {
        List<String> packs = new ArrayList<>();
        List<String> packNames = new ArrayList<>();
        List<List<String>> packItems = new ArrayList<>();
        List<Category> kaomoji = new ArrayList<>();
        if (groups == null) return List.of();
        for (int index = 0; index < groups.size(); index++) {
            PluginGroup group = groups.get(index);
            if (group == null || !usableName(group.pack()) || !usableName(group.packName()) || !usableName(group.title())
                    || group.items() == null) continue;
            List<String> items = new ArrayList<>();
            for (String item : group.items()) {
                if (items.size() == MAX_PLUGIN_GROUP_ITEMS) break;
                if (item != null && !item.isEmpty() && !item.trim().isEmpty() && item.length() <= MAX_PLUGIN_ITEM_UNITS
                        && item.indexOf('\u0000') < 0) items.add(item);
            }
            if (items.isEmpty()) continue;
            if (SYMBOLS_CATALOG.equals(group.tab())) {
                int pack = packs.indexOf(group.pack());
                if (pack < 0) {
                    packs.add(group.pack());
                    packNames.add(group.packName());
                    packItems.add(new ArrayList<>());
                    pack = packs.size() - 1;
                }
                packItems.get(pack).addAll(items);
            } else if (KAOMOJI_CATALOG.equals(group.tab())) {
                kaomoji.add(new Category(PLUGIN_KAOMOJI_PREFIX + group.pack() + ":" + index, group.title(), items, "", "",
                    true));
            }
        }
        List<Category> values = new ArrayList<>(packs.size() + kaomoji.size());
        for (int pack = 0; pack < packs.size(); pack++) {
            values.add(new Category(PLUGIN_SYMBOLS_PREFIX + packs.get(pack), packNames.get(pack), packItems.get(pack)));
        }
        values.addAll(kaomoji);
        return List.copyOf(values);
    }

    private static boolean usableName(String value) {
        return value != null && !value.isEmpty() && value.length() <= MAX_PLUGIN_NAME_UNITS && !value.trim().isEmpty();
    }

    /**
     * 按本机分类顺序排好的内置分类，再插进符号集插件的分类：颜文字插件组紧跟在「颜文字」之后（鸿蒙 2in1 和桌面面板把它们排在颜文字的「All」之后，这个面板的「颜文字」就是 All），符号插件排在最后，也就是内置符号之后。插件分类不进「符号分类顺序」，用户调整顺序时它们跟着「颜文字」和末尾走。
     */
    public static List<Category> categories(List<String> recents, List<SymbolCategoryOrderPolicy.Item> order,
            List<Category> plugins) {
        List<Category> values = new ArrayList<>(categories(recents, order));
        if (plugins == null || plugins.isEmpty()) return List.copyOf(values);
        int kaomoji = -1;
        for (int index = 0; index < values.size(); index++) if ("kaomoji".equals(values.get(index).id())) kaomoji = index;
        int insert = kaomoji < 0 ? values.size() : kaomoji + 1;
        List<Category> symbols = new ArrayList<>();
        for (Category category : plugins) {
            if (category.kaomoji()) values.add(insert++, category);
            else symbols.add(category);
        }
        values.addAll(symbols);
        return List.copyOf(values);
    }

    // 「常用」只显示用户在面板里实际点过的符号，固定标点和全角符号留在「中文」最前面。
    // 每行五格，成对的括号和引号两两占左边四格、右边一格放单个符号，前后两半不会分在两行（{@link #breaksBeforePair}）；顺序与 iOS、鸿蒙的「中文」相同。
    private static final List<Category> FIXED_CATEGORIES = List.of(
        new Category("chinese", "中文", List.of(
            "，", "。", "？", "！", "、",
            "；", "：", "…", "—", "·",
            "“", "”", "‘", "’", "～",
            "（", "）", "《", "》", "＆",
            "【", "】", "＃", "＠", "％",
            "＋", "－", "＝", "／", "￥",
            "〈", "〉", "「", "」", "〇",
            "『", "』", "〔", "〕", "※",
            "〖", "〗", "＜", "＞", "°",
            "｛", "｝", "［", "］", "℃",
            "︵", "︶", "﹁", "﹂", "±",
            "×", "÷", "≈", "≠", "≤",
            "≥", "√", "∞", "∵", "∴",
            "→", "←", "↑", "↓", "★",
            "☆", "●", "○", "■", "□",
            "◆", "◇", "▲", "△")),
        new Category("radicals", "偏旁", List.of()),
        new Category("english", "英文", List.of(
            ",", ".", "?", "!", ";", ":", "'", "\"", "(", ")",
            "[", "]", "{", "}", "/", "<", ">", "\\", "|", "-",
            "_", "+", "=", "*", "&", "^", "%", "$", "#", "@",
            "~", "`", "·", "…", "–", "—", "§", "¶", "†", "‡")),
        new Category("numbers", "数字", List.of(
            "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
            "①", "②", "③", "④", "⑤", "⑥", "⑦", "⑧", "⑨", "⑩",
            "一", "二", "三", "四", "五", "六", "七", "八", "九", "十",
            "Ⅰ", "Ⅱ", "Ⅲ", "Ⅳ", "Ⅴ", "Ⅵ", "Ⅶ", "Ⅷ", "Ⅸ", "Ⅹ",
            "½", "⅓", "¼", "‰", "′", "″", "㎡", "㎏", "㎝", "№")),
        new Category("network", "网络", network()),
        new Category("pinyin", "拼音", List.of(
            "ā", "á", "ǎ", "à", "ō", "ó", "ǒ", "ò", "ê", "ē", "é", "ě", "è",
            "ī", "í", "ǐ", "ì", "ū", "ú", "ǔ", "ù", "ü", "ǖ", "ǘ", "ǚ", "ǜ", "ń", "ň")));

    /**
     * 写死的分类之后，是 Engine 目录里的颜文字和各类符号（#5667「文本符号过少」）。数据随词库 `msime-others.db` 发布，与 iOS 键盘和桌面面板读的是同一份；上级分类的中文名与 iOS 的 `KeyboardEmojiCatalog.symbolParentTitles` 相同，顺序按目录里的顺序。
     */
    private static final List<Category> CATALOG_CATEGORIES = List.of(
        new Category("kaomoji", "颜文字", List.of(), KAOMOJI_CATALOG, ""),
        new Category("shapes", "形状", List.of(), SYMBOLS_CATALOG, "Stars and shapes"),
        new Category("arrows", "箭头", List.of(), SYMBOLS_CATALOG, "Arrows and lines"),
        new Category("punctuation", "标点", List.of(), SYMBOLS_CATALOG, "Punctuation"),
        new Category("math", "数学", List.of(), SYMBOLS_CATALOG, "Math"),
        new Category("currency", "货币", List.of(), SYMBOLS_CATALOG, "Currency"),
        new Category("hearts", "爱心", List.of(), SYMBOLS_CATALOG, "Hearts"),
        new Category("letters", "字母", List.of(), SYMBOLS_CATALOG, "Letters"),
        new Category("games", "游戏", List.of(), SYMBOLS_CATALOG, "Games"),
        new Category("culture", "文化", List.of(), SYMBOLS_CATALOG, "Culture"),
        new Category("nature", "自然", List.of(), SYMBOLS_CATALOG, "Animals and nature"),
        new Category("people", "人物", List.of(), SYMBOLS_CATALOG, "People and activity"),
        new Category("more", "更多", List.of(), SYMBOLS_CATALOG, "More"));

    private SymbolPanelModel() { }

    /** 「网络」分类：网址和常用符号，后面接全部邮箱后缀（{@link EmailSuffixPolicy#SUFFIXES}，#6147），和输入框里打到 `xxx@` 时候选栏给的是同一份。 */
    private static List<String> network() {
        ArrayList<String> values = new ArrayList<>(List.of(
            "@", "#", "/", "\\", ":", "_", "-", "+", "=", "&",
            "?", "%", "~", "^", "*", "|", "<", ">", "$", "€",
            "http://", "https://", "www.", ".com", ".cn", ".net", ".org", ".io"));
        values.addAll(EmailSuffixPolicy.SUFFIXES);
        return values;
    }

    /**
     * 符号格里的一个条目是否按字形墨迹居中（{@code CenteredGlyphSpan}）。
     *
     * <p>只有全角/CJK 标点的墨迹偏在字身一角，需要按墨迹居中才不贴着格子边；其余字形的高低是字形本身的
     * 意义，按字体基线排：半角 ASCII 的 `_` 才能落在 `-` 下方，目录里的 `₀` 与 `⁰`、`‚` 与 `‛`、
     * `⏝` 与 `⏞`、`▁` 与 `▔` 也不会叠在同一高度。多字条目和颜文字不居中。
     */
    public static boolean centersGlyphInk(String symbol) {
        if (symbol == null || TextPolicy.codePointLength(symbol) != 1) return false;
        int codePoint = symbol.codePointAt(0);
        // 字母、数字和全角字母数字都按基线排，半角假名和全角数字才不会因为字身高低不同而上下跳。
        if (Character.isLetterOrDigit(codePoint)) return false;
        // 这些码位在全角区段里，但高低本身就是字形的一部分（垂直引号、上/下线、全角长音符），不拉回中线。
        if ((codePoint >= 0x301D && codePoint <= 0x301F)
                || (codePoint >= 0xFE49 && codePoint <= 0xFE4F)
                || codePoint == 0xFF3F || codePoint == 0xFFE3) return false;
        return (codePoint >= 0x3000 && codePoint <= 0x303F)
            || (codePoint >= 0xFE30 && codePoint <= 0xFE4F)
            || (codePoint >= 0xFF00 && codePoint <= 0xFFEF)
            || codePoint == 0x2014
            || codePoint == 0x2018 || codePoint == 0x2019 || codePoint == 0x201C || codePoint == 0x201D
            || codePoint == 0x2026 || codePoint == 0x00B7;
    }

    /**
     * 符号格里一个条目的字号（sp）：颜文字 14；不超过 {@link #SHORT_SYMBOL_CODE_POINTS} 个码位的 18；更长的条目在 9 个码位以内用 13，再长用 11；固定分类的长条目按实际字宽跨列，保持单行。
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

    /**
     * 排到 `column` 列的 `symbols[index]` 是成对符号的前半个、又正好在行尾，而后半个紧跟在它后面时为真：这一格留空，两半一起排到下一行，（ 和 ） 不分在两行。成对与否按 {@link PairedPunctuationPolicy#symbolClosing}，和点前半个时补后半个是同一张表。「常用」的顺序是点的先后，目录和插件的顺序来自数据，都只能在排版时处理。
     */
    public static boolean breaksBeforePair(List<String> symbols, int index, int column, int columns) {
        if (column != columns - 1 || index + 1 >= symbols.size()) return false;
        String closing = PairedPunctuationPolicy.symbolClosing(symbols.get(index));
        return closing != null && closing.equals(symbols.get(index + 1));
    }

    /** 每个条目占一格时，按 {@link #breaksBeforePair} 留出的空格数；写死的分类排好了顺序，应当为 0。 */
    public static int pairGaps(List<String> symbols, int columns) {
        int gaps = 0, column = 0;
        for (int index = 0; index < symbols.size(); index++) {
            if (breaksBeforePair(symbols, index, column, columns)) {
                gaps++;
                column = 0;
            }
            column = (column + 1) % columns;
        }
        return gaps;
    }

    /** 非颜文字分类里的长条目至少跨两列，再按实际字宽增加跨度；目录分页也按实际字宽排版。 */
    public static int cellColumnSpan(Category category, String symbol, float textWidth,
            float columnWidth, float cellInset) {
        if (category.kaomoji() || !singleLineCell(category, symbol)) return 1;
        int measured = columnWidth > 0 ? (int) Math.ceil((textWidth + cellInset) / columnWidth) : 2;
        return Math.min(category.columns(), Math.max(2, measured));
    }

    /** 底栏的全部分类：第一个是「常用」（{@link #commonSymbols}），然后是写死的分类，最后是 Engine 目录里的分类。 */
    public static List<Category> categories(List<String> recents) {
        ArrayList<Category> values = new ArrayList<>(FIXED_CATEGORIES.size() + 1);
        values.add(new Category("recents", RECENTS_TITLE, commonSymbols(recents)));
        values.addAll(FIXED_CATEGORIES);
        values.addAll(CATALOG_CATEGORIES);
        return List.copyOf(values);
    }

    /** 按本机设置排列分类；设置中缺少的分类（例如后续版本新增的分类）追加到默认顺序末尾。 */
    public static List<Category> categories(List<String> recents,
            List<SymbolCategoryOrderPolicy.Item> order) {
        List<Category> defaults = categories(recents);
        if (order == null || order.equals(SymbolCategoryOrderPolicy.DEFAULT_ORDER)) return defaults;
        ArrayList<Category> values = new ArrayList<>(defaults.size());
        for (SymbolCategoryOrderPolicy.Item item : order) {
            for (Category category : defaults) {
                if (category.id().equals(item.id) && !values.contains(category)) {
                    values.add(category);
                    break;
                }
            }
        }
        for (Category category : defaults) if (!values.contains(category)) values.add(category);
        return List.copyOf(values);
    }

    /** 「常用」只显示实际使用记录，最近点过的在前，不追加默认符号。 */
    public static List<String> commonSymbols(List<String> recents) {
        return normalizeRecents(recents);
    }

    /** 打开面板时总是显示「常用」，没有记录时显示空态提示。 */
    public static int initialCategory(List<String> recents) {
        return 0;
    }

    /** 「常用」移动后仍按稳定 ID 找到它，而不是假设它位于索引 0。 */
    public static int initialCategory(List<String> recents,
            List<SymbolCategoryOrderPolicy.Item> order) {
        return initialCategory(recents, order, List.of());
    }

    /** 同上，算上符号集插件的分类：插到「常用」之前的颜文字插件组会挪动它的下标。 */
    public static int initialCategory(List<String> recents,
            List<SymbolCategoryOrderPolicy.Item> order, List<Category> plugins) {
        List<Category> values = categories(recents, order, plugins);
        for (int index = 0; index < values.size(); index++)
            if ("recents".equals(values.get(index).id())) return index;
        return 0;
    }

    /** 读回存储的记录：丢掉空串、超长条目和重复项，最多保留 {@link #RECENTS_LIMIT} 个。 */
    public static List<String> normalizeRecents(List<String> stored) {
        return ListPolicy.normalizeRecents(stored, RECENTS_LIMIT, SymbolPanelModel::validSymbol);
    }

    /** 点了 `selected` 之后的记录：它排到最前，原来的位置去掉，超出上限的最旧一个被挤掉。 */
    public static List<String> recordRecent(List<String> stored, String selected) {
        if (!validSymbol(selected)) throw new IllegalArgumentException("Invalid recent symbol");
        return ListPolicy.recordRecent(stored, selected, RECENTS_LIMIT,
            SymbolPanelModel::validSymbol);
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
        return BoundsPolicy.validPageCursor(requestedOffset, count, CATALOG_PAGE_SIZE,
            nextOffset, complete);
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

}
