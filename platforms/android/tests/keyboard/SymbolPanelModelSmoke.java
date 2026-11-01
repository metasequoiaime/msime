import app.msime.android.SymbolCategoryOrderPolicy;
import app.msime.android.SymbolPanelModel;

import java.util.ArrayList;
import java.util.List;

public final class SymbolPanelModelSmoke {
    public static void main(String[] args) {
        List<SymbolPanelModel.Category> empty = SymbolPanelModel.categories(List.of());
        check(empty.size() == 7 + 13, "seven fixed categories, kaomoji and twelve symbol parents: " + empty.size());
        check(empty.get(0).title().equals("常用") && empty.get(0).symbols().isEmpty(),
            "没有使用记录时常用为空");
        check(empty.get(5).symbols().get(20).equals("http://"), "network shortcuts");
        check(empty.get(6).title().equals("拼音") && empty.get(6).symbols().equals(List.of(
                "ā", "á", "ǎ", "à", "ō", "ó", "ǒ", "ò", "ê", "ē", "é", "ě", "è",
                "ī", "í", "ǐ", "ì", "ū", "ú", "ǔ", "ù", "ü", "ǖ", "ǘ", "ǚ", "ǜ", "ń", "ň")),
            "pinyin tone symbols");
        // 原先只在「常用」里的中文标点挪到了「中文」，没有从面板里消失。
        for (String mark : List.of("，", "。", "？", "！", "、", "“", "”", "（", "）", "《", "》", "【", "】", "＠", "／"))
            check(empty.get(1).symbols().contains(mark), "中文 keeps " + mark);
        for (SymbolPanelModel.Category category : empty)
            check(category.symbols().size() == new java.util.HashSet<>(category.symbols()).size(),
                category.title() + " has no duplicates");
        check(empty.get(0).id().equals("recents") && empty.get(1).id().equals("chinese")
            && empty.get(19).id().equals("more"), "categories expose stable IDs");
        catalogCategories(empty);
        networkSuffixes(empty.get(5));
        glyphCentering();
        pairsStayOnOneRow(empty);

        check(SymbolPanelModel.initialCategory(List.of()) == 0, "没有记录时仍打开常用");
        check(SymbolPanelModel.initialCategory(List.of("＞")) == 0, "history opens on 常用");

        // 点过的符号排到最前，重复点不会出现两次。
        List<String> recents = SymbolPanelModel.recordRecent(List.of(), "＞");
        recents = SymbolPanelModel.recordRecent(recents, "。");
        recents = SymbolPanelModel.recordRecent(recents, "＞");
        check(recents.equals(List.of("＞", "。")), "most recent first, deduplicated: " + recents);
        List<String> common = SymbolPanelModel.categories(recents).get(0).symbols();
        check(common.equals(recents), "常用只显示实际记录，不补入默认符号：" + common);

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

        System.out.println("SymbolPanelModelSmoke: PASS");
    }

    /** 成对符号的两半不分在两行：写死的分类本身排好了不留空格，「常用」这类顺序不定的列表在排版时把前半个挪到下一行。 */
    private static void pairsStayOnOneRow(List<SymbolPanelModel.Category> categories) {
        for (SymbolPanelModel.Category category : categories) {
            if (category.fromCatalog() || category.symbols().isEmpty()) continue;
            check(SymbolPanelModel.pairGaps(category.symbols(), category.columns()) == 0,
                category.title() + " keeps every pair on one row without gaps");
        }
        check(categories.get(1).symbols().size() == 79, "中文 keeps all 79 symbols: " + categories.get(1).symbols().size());
        List<String> recents = List.of("，", "。", "？", "！", "（", "）", "《");
        check(SymbolPanelModel.breaksBeforePair(recents, 4, 4, 5), "（ at the end of a row moves down with ）");
        check(SymbolPanelModel.pairGaps(recents, 5) == 1, "常用 leaves one gap before a split pair");
        check(!SymbolPanelModel.breaksBeforePair(recents, 4, 3, 5), "（ before the last column stays");
        check(!SymbolPanelModel.breaksBeforePair(List.of("，", "。", "？", "！", "（", "。"), 4, 4, 5),
            "（ not followed by ） stays at the end of the row");
        check(!SymbolPanelModel.breaksBeforePair(List.of("，", "。", "？", "！", "（"), 4, 4, 5),
            "a trailing （ has nothing to keep it with");
        check(!SymbolPanelModel.breaksBeforePair(List.of("，", "。", "？", "！", "）", "（"), 4, 4, 5),
            "a closing half is never the start of a pair");
        check(SymbolPanelModel.pairGaps(List.of("“", "”", "‘", "’", "（", "）"), 5) == 1
            && SymbolPanelModel.pairGaps(List.of("a", "“", "”", "‘", "’", "（", "）"), 5) == 0,
            "only a pair starting in the last column is moved");
    }

    /** 只有全角/CJK 标点按墨迹居中；靠高低表达含义的字形全部按字体基线排。 */
    private static void glyphCentering() {
        for (String baseline : List.of("-", "_", ".", ",", "\"", "'", ";", ":", "|", "~", "`", "@", "0", "A",
                "一", "５", "Ⅰ", "①", "ｦ", "㋿",
                "‗", "₀", "₉", "₊", "ₐ", "⁰", "²", "⁹", "ⁿ", "ⁱ", "′", "″", "‴", "‵", "‚", "„", "‛", "‟",
                "¨", "¯", "´", "¸", "º", "ª", "⁎", "‥", "⁏", "▁", "▄", "▀", "▔", "⌃", "⌄", "⌢", "⌣",
                "⏜", "⏝", "⏞", "⏟", "⏠", "⏡", "⎴", "⎵", "‿", "⁀", "﹉", "﹍", "﹎", "﹏", "＿", "￣", "〝", "〞", "〟"))
            check(!SymbolPanelModel.centersGlyphInk(baseline), "positional glyphs keep the font baseline: " + baseline);
        for (String fullWidth : List.of("，", "。", "、", "：", "；", "！", "？", "“", "”", "（", "）", "《", "》",
                "【", "】", "「", "」", "＠", "－", "～", "…", "—", "·", "︵", "﹁"))
            check(SymbolPanelModel.centersGlyphInk(fullWidth), "full-width punctuation centres its ink: " + fullWidth);
        for (String longEntry : new String[] {"@protonmail.com", "http://", "（笑）", "😀😀"})
            check(!SymbolPanelModel.centersGlyphInk(longEntry), "multi-glyph entries stay on the baseline: " + longEntry);
        check(!SymbolPanelModel.centersGlyphInk(null) && !SymbolPanelModel.centersGlyphInk(""),
            "missing symbols are not drawn");
    }

    /** #6147：「网络」接上全部邮箱后缀，国内常用的在前；长条目缩小字号并单行省略。 */
    private static void networkSuffixes(SymbolPanelModel.Category network) {
        check(network.title().equals("网络"), "network is the fifth category");
        List<String> symbols = network.symbols();
        check(symbols.size() == 48, "network has 28 shortcuts and 20 suffixes: " + symbols.size());
        for (String suffix : List.of("@163.com", "@126.com", "@139.com", "@189.cn", "@aliyun.com", "@foxmail.com",
                "@outlook.com", "@hotmail.com", "@live.com", "@icloud.com", "@yahoo.com", "@proton.me",
                "@protonmail.com", "@aol.com", "@mail.com", "@gmx.com", "@naver.com", "@yahoo.co.jp",
                "@qq.com", "@gmail.com"))
            check(symbols.contains(suffix), "network offers " + suffix);
        check(symbols.indexOf("@foxmail.com") < symbols.indexOf("@gmail.com")
            && symbols.indexOf("@aliyun.com") < symbols.indexOf("@outlook.com"), "domestic suffixes come first");
        check(symbols.subList(28, 48).equals(app.msime.android.EmailSuffixPolicy.SUFFIXES),
            "the panel and the candidate bar share one suffix list");
        for (String symbol : symbols)
            check(symbol.codePointCount(0, symbol.length()) <= SymbolPanelModel.MAX_SYMBOL_CODE_POINTS,
                symbol + " fits in 常用");
        check(SymbolPanelModel.cellTextSizeSp(network, "@") == 18 && !SymbolPanelModel.singleLineCell(network, "@"),
            "a single symbol keeps the normal size");
        check(SymbolPanelModel.cellTextSizeSp(network, ".com") == 18 && !SymbolPanelModel.singleLineCell(network, ".com"),
            "four characters still fit");
        check(SymbolPanelModel.cellTextSizeSp(network, "@163.com") == 13 && SymbolPanelModel.singleLineCell(network, "@163.com"),
            "a short suffix shrinks and stays on one line");
        check(SymbolPanelModel.cellTextSizeSp(network, "@protonmail.com") == 11
            && SymbolPanelModel.singleLineCell(network, "@protonmail.com"), "a long suffix shrinks further");
        check(SymbolPanelModel.cellColumnSpan(network, "@qq.com", 42, 60, 12) == 2,
            "a suffix gets a wider key even when two columns suffice");
        check(SymbolPanelModel.cellColumnSpan(network, "@protonmail.com", 112, 40, 12) == 4,
            "small screens widen long domains according to measured text");
        check(SymbolPanelModel.cellColumnSpan(network, "@protonmail.com", 112, 0, 12) == 2,
            "initial layout has a deterministic wide fallback");
        check(SymbolPanelModel.cellColumnSpan(network, "@protonmail.com", 112, 10, 12) == 5,
            "span never exceeds the grid");
        check(SymbolPanelModel.cellColumnSpan(network, "@", 8, 60, 12) == 1,
            "single symbols keep their compact cells");
        check(SymbolPanelModel.cellColumnSpan(SymbolPanelModel.categories(List.of()).get(0), "@gmail.com", 80, 60, 12) == 2,
            "recents use the same wider suffix rule");
        check(SymbolPanelModel.cellColumnSpan(SymbolPanelModel.categories(List.of()).get(8), "(long symbol)", 80, 60, 12) == 2,
            "catalog symbols use the measured width");
        check(SymbolPanelModel.cellColumnSpan(SymbolPanelModel.categories(List.of()).get(7), "(long face)", 112, 60, 12) == 1,
            "kaomoji keep their two-column layout");

    }

    /** #5667：颜文字和 Engine 符号目录接在写死的分类后面。 */
    private static void catalogCategories(List<SymbolPanelModel.Category> categories) {
        for (int index = 0; index < 7; index++) {
            SymbolPanelModel.Category fixed = categories.get(index);
            check(!fixed.fromCatalog() && fixed.columns() == SymbolPanelModel.COLUMNS && fixed.remembers(),
                fixed.title() + " is a fixed five-column category");
        }
        SymbolPanelModel.Category kaomoji = categories.get(7);
        check(kaomoji.title().equals("颜文字") && kaomoji.kaomoji() && kaomoji.fromCatalog()
            && kaomoji.symbols().isEmpty(), "颜文字 comes from the Engine catalog");
        check(kaomoji.columns() == SymbolPanelModel.KAOMOJI_COLUMNS && !kaomoji.remembers(),
            "kaomoji are two per row and stay out of 常用");
        java.util.HashSet<String> parents = new java.util.HashSet<>();
        for (SymbolPanelModel.Category category : categories.subList(8, categories.size())) {
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

        // #6070：设备字体画不出来的符号不显示，否则是一格空白、点了上屏一个看不见的字；颜文字是多个字形，不按单字形判断。
        java.util.Set<String> missing = java.util.Set.of("⭠", "🩷");
        check(SymbolPanelModel.renderableCatalogItems(List.of("⬅", "⭠", "❤", "🩷", "❤️"), false,
            text -> !missing.contains(text)).equals(List.of("⬅", "❤", "❤️")), "undrawable symbols are dropped");
        check(SymbolPanelModel.renderableCatalogItems(List.of("⭠"), false, text -> false).isEmpty(),
            "a page with nothing drawable is an empty page, not a failure");
        check(SymbolPanelModel.renderableCatalogItems(List.of("(ノ°▽°)ノ", "ʕ•ᴥ•ʔ"), true, text -> false)
            .equals(List.of("(ノ°▽°)ノ", "ʕ•ᴥ•ʔ")), "kaomoji are kept: hasGlyph only answers single glyphs");
        // 过滤上线前点过的空白格已经在「常用」里；多字形的条目 hasGlyph 一律回答「不能」，只判断单个码位（可带变体选择符）。
        java.util.Set<String> undrawable = java.util.Set.of("⭠", "🩷", "🩵️");
        java.util.Set<String> judged = new java.util.HashSet<>();
        List<String> recentsShown = SymbolPanelModel.renderableRecents(
            List.of("⭠", "，", "🩷", "http://", "@gmail.com", "🩵️", "❤️", "❤️‍🔥"),
            text -> { judged.add(text); return !undrawable.contains(text) && text.codePointCount(0, text.length()) <= 2; });
        check(recentsShown.equals(List.of("，", "http://", "@gmail.com", "❤️", "❤️‍🔥")),
            "undrawable single symbols leave 常用, multi-glyph entries stay: " + recentsShown);
        check(!judged.contains("http://") && !judged.contains("❤️‍🔥") && judged.contains("❤️"),
            "only single code points (with an optional variation selector) are judged: " + judged);
        pluginCategories();
    }

    /** 符号集插件：符号组按插件合成一个分类排在最后，颜文字组逐组紧跟在「颜文字」之后，坏组跳过。 */
    private static void pluginCategories() {
        List<SymbolPanelModel.PluginGroup> groups = List.of(
            new SymbolPanelModel.PluginGroup("arrows", "我的箭头", "symbols", "箭头", List.of("→", "←")),
            new SymbolPanelModel.PluginGroup("faces", "表情", "kaomoji", "开心", List.of("(^_^)", "")),
            new SymbolPanelModel.PluginGroup("arrows", "我的箭头", "symbols", "斜箭头", List.of("↗", "x".repeat(65))),
            new SymbolPanelModel.PluginGroup("bad", "坏", "emoji", "未知的页", List.of("x")),
            new SymbolPanelModel.PluginGroup("empty", "空", "symbols", "空组", List.of(" ")),
            new SymbolPanelModel.PluginGroup("faces", "表情", "kaomoji", "难过", List.of("(T_T)")));
        List<SymbolPanelModel.Category> plugins = SymbolPanelModel.pluginCategories(groups);
        check(plugins.size() == 3, "one symbols category per pack, one kaomoji category per group: " + plugins.size());
        SymbolPanelModel.Category arrows = plugins.get(0);
        check(arrows.title().equals("我的箭头") && arrows.symbols().equals(List.of("→", "←", "↗"))
            && !arrows.kaomoji() && arrows.plugin() && arrows.remembers() && !arrows.fromCatalog()
            && arrows.columns() == SymbolPanelModel.COLUMNS, "a pack's symbols groups are one category, overlong items dropped");
        check(plugins.get(1).title().equals("开心") && plugins.get(1).kaomoji() && !plugins.get(1).remembers()
            && plugins.get(1).columns() == SymbolPanelModel.KAOMOJI_COLUMNS && plugins.get(1).symbols().equals(List.of("(^_^)")),
            "a kaomoji group is laid out like kaomoji and not remembered");
        check(!plugins.get(1).id().equals(plugins.get(2).id()), "two kaomoji groups of one pack have distinct ids");

        List<SymbolPanelModel.Category> all = SymbolPanelModel.categories(List.of(), SymbolCategoryOrderPolicy.DEFAULT_ORDER, plugins);
        List<String> ids = new java.util.ArrayList<>();
        for (SymbolPanelModel.Category category : all) ids.add(category.id());
        int kaomoji = ids.indexOf("kaomoji");
        check(ids.get(kaomoji + 1).equals(plugins.get(1).id()) && ids.get(kaomoji + 2).equals(plugins.get(2).id()),
            "kaomoji groups follow 颜文字: " + ids);
        check(ids.get(ids.size() - 1).equals(arrows.id()), "symbols packs come after the built-in categories: " + ids);
        check(SymbolPanelModel.categories(List.of(), SymbolCategoryOrderPolicy.DEFAULT_ORDER, List.of())
            .equals(SymbolPanelModel.categories(List.of(), SymbolCategoryOrderPolicy.DEFAULT_ORDER)), "no plugins, no change");
        check(SymbolPanelModel.initialCategory(List.of(), SymbolCategoryOrderPolicy.DEFAULT_ORDER, plugins)
            == SymbolPanelModel.initialCategory(List.of(), SymbolCategoryOrderPolicy.DEFAULT_ORDER), "常用 stays the first page");
        check(SymbolPanelModel.pluginCategories(null).isEmpty(), "no answer, no plugin categories");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
