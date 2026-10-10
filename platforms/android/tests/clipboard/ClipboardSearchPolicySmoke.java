import app.msime.android.ClipboardHistory;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.ClipboardSearchPolicy;
import java.util.List;
import java.util.Locale;

/** #5973：应用里可搜索的剪贴板历史页按查询筛条目，顺序不变，筛不出时说出查询。 */
public final class ClipboardSearchPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static String texts(List<ClipboardHistory.Item> items) {
        StringBuilder out = new StringBuilder(items.size() * 16);
        for (ClipboardHistory.Item item : items) {
            if (out.length() > 0) out.append('|');
            out.append(item.text());
        }
        return out.toString();
    }

    public static void main(String[] args) {
        // 共享存储给的顺序：置顶在前，然后从新到旧。
        List<ClipboardHistory.Item> items = List.of(
            new ClipboardHistory.Item("合成置顶 Alpha 地址", 300, true),
            new ClipboardHistory.Item("https://example.test/Synthetic?id=1", 500, false),
            new ClipboardHistory.Item("合成会议纪要第二段", 400, false),
            new ClipboardHistory.Item("synthetic alpha note", 200, false));

        // 空查询和只有空白的查询显示全部，顺序不变，也不是同一个列表对象。
        check(texts(ClipboardSearchPolicy.filter(items, "")).equals(texts(items)), "an empty query shows everything");
        check(texts(ClipboardSearchPolicy.filter(items, null)).equals(texts(items)), "a null query shows everything");
        check(texts(ClipboardSearchPolicy.filter(items, "  \t")).equals(texts(items)), "a blank query shows everything");
        check(ClipboardSearchPolicy.filter(items, "") != items, "the result is a copy the page may keep");

        // 不区分大小写的子串匹配，置顶的那条照样排在前面。
        check(texts(ClipboardSearchPolicy.filter(items, "ALPHA"))
                .equals("合成置顶 Alpha 地址|synthetic alpha note"),
            "matching ignores case and keeps the pinned entry first");
        check(texts(ClipboardSearchPolicy.filter(items, "synthetic"))
                .equals("https://example.test/Synthetic?id=1|synthetic alpha note"),
            "matches keep the shared store's order");
        // 中文子串、首尾空白被去掉、查询中间的空格保留。
        check(texts(ClipboardSearchPolicy.filter(items, "会议")).equals("合成会议纪要第二段"), "a Chinese substring matches");
        check(texts(ClipboardSearchPolicy.filter(items, "  纪要 ")).equals("合成会议纪要第二段"), "surrounding blanks are ignored");
        check(texts(ClipboardSearchPolicy.filter(items, "alpha note")).equals("synthetic alpha note"), "inner spaces are part of the query");
        check(ClipboardSearchPolicy.filter(items, "alphanote").isEmpty(), "inner spaces are not dropped");
        check(texts(ClipboardSearchPolicy.filter(items, "?ID=1")).equals("https://example.test/Synthetic?id=1"),
            "punctuation is matched literally");
        check(ClipboardSearchPolicy.filter(items, "不存在的词").isEmpty(), "an unmatched query finds nothing");
        check(ClipboardSearchPolicy.filter(List.of(), "alpha").isEmpty(), "an empty history finds nothing");

        // 按 Locale.ROOT 转小写：系统是土耳其语时 `I` 也照样匹配 `i`。
        Locale previous = Locale.getDefault();
        try {
            Locale.setDefault(Locale.forLanguageTag("tr-TR"));
            check(texts(ClipboardSearchPolicy.filter(items, "SYNTHETIC ALPHA")).equals("synthetic alpha note"),
                "case folding does not follow the device locale");
        } finally {
            Locale.setDefault(previous);
        }

        // 筛不出时说出查询，去掉首尾空白。
        check(ClipboardSearchPolicy.emptyMessage("  会议 ").equals("没有包含“会议”的记录"), "the empty message names the query");
        check(ClipboardSearchPolicy.query(null).isEmpty(), "a null query reads as empty");

        try {
            ClipboardSearchPolicy.filter(null, "alpha");
            throw new AssertionError("a missing history is refused");
        } catch (IllegalArgumentException expected) {
            // 页面读不出历史时走自己的失败提示，不该把 null 交到这里。
        }

        // 在应用里复制一条后键盘把它重新记下：共享存储换了时间戳并挪到最前，页面列表里还是旧的。编辑用的键要按现在的时间戳算，否则编辑页找不到它。
        ClipboardHistory.Item shown = items.get(2);
        List<ClipboardHistory.Item> recaptured = List.of(
            items.get(0),
            new ClipboardHistory.Item("合成会议纪要第二段", 900, false),
            items.get(1),
            items.get(3));
        String stale = ClipboardHistoryPolicy.editKey(shown.timestamp(), shown.text());
        String current = ClipboardSearchPolicy.currentEditKey(recaptured, shown.text());
        check(ClipboardHistoryPolicy.editKey(900, "合成会议纪要第二段").equals(current),
            "the edit key follows the entry's current timestamp");
        check(!stale.equals(current), "the stale list's key would no longer match");
        check(ClipboardSearchPolicy.currentEditKey(recaptured, "已经删掉的合成记录") == null,
            "a removed entry has no key");
        check(ClipboardSearchPolicy.currentEditKey(recaptured, "合成会议纪要") == null,
            "the entry is found by its whole text, not a prefix");
        check(ClipboardSearchPolicy.currentEditKey(null, "合成会议纪要第二段") == null, "an unreadable history has no key");

        // 键盘写死的页面名要和 PageId 的枚举名同形；PageId 本身进不了 JVM 编译，两边一致由 check-host.sh 核对。
        check(ClipboardSearchPolicy.SEARCH_PAGE.matches("[A-Z][A-Z0-9_]*"), "the page name is a PageId constant name");
        System.out.println("ClipboardSearchPolicySmoke ok");
    }
}
