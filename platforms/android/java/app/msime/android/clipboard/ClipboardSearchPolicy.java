package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 应用里可搜索的剪贴板历史页（#5973）按查询筛哪几条、筛不出时说什么。不依赖 Android，JVM 冒烟直接测；页面本身在 `home/ClipboardSearchPage`，只在 Gradle 构建里编译。
 *
 * <p>查询去掉首尾空白后按 {@link Locale#ROOT} 转小写，和每条文字转小写后做子串匹配；不按系统语言转换，土耳其语环境下 `I` 也照样匹配 `i`。结果保留共享存储给的顺序（置顶在前，然后按时间），不按匹配程度重排：本机历史最多 {@link ClipboardHistoryPolicy#LIMIT} 条，用户按记得的位置找。空查询返回全部。
 */
public final class ClipboardSearchPolicy {
    /** 键盘剪贴板面板顶行「搜索」打开的页面名（`PageId` 的枚举名）；键盘进程不能引用 `home/` 的类，所以写成字符串，`check-host.sh` 核对两边一致。 */
    public static final String SEARCH_PAGE = "CLIPBOARD_SEARCH";

    private ClipboardSearchPolicy() {}

    /** 实际用来匹配的查询：去掉首尾空白，null 当作空。 */
    public static String query(String raw) {
        return TextPolicy.trimmed(raw);
    }

    /** 按查询筛出的条目，顺序不变；查询为空时是全部条目的副本。 */
    public static List<ClipboardHistory.Item> filter(List<ClipboardHistory.Item> items, String raw) {
        if (items == null) throw new IllegalArgumentException("No clipboard history");
        String needle = query(raw).toLowerCase(Locale.ROOT);
        List<ClipboardHistory.Item> matched = new ArrayList<>(items.size());
        for (ClipboardHistory.Item item : items) {
            if (needle.isEmpty() || item.text().toLowerCase(Locale.ROOT).contains(needle)) matched.add(item);
        }
        return matched;
    }

    /**
     * 打开编辑页时交给它的键：按文字在刚读到的历史里找那一条，用它现在的时间戳算 {@link ClipboardHistoryPolicy#editKey}；那一条已经不在时为 null。
     *
     * <p>页面上的列表可能已经旧了：在应用里点按一条复制之后，键盘的复制监听会把它重新记一遍，共享存储给它换上新的时间戳并挪到最前，而这一页还显示着旧的。按列表里的旧时间戳算键，编辑页会说这一条已经不在了。共享存储按文字认条目（同一段文字只有一条），所以按文字找回现在的那一条。
     */
    public static String currentEditKey(List<ClipboardHistory.Item> items, String text) {
        if (items == null || text == null) return null;
        for (ClipboardHistory.Item item : items) {
            if (item.text().equals(text)) return ClipboardHistoryPolicy.editKey(item.timestamp(), item.text());
        }
        return null;
    }

    /** 有查询但一条也没筛出来时显示的话，带上用户输入的查询（去掉首尾空白）。 */
    public static String emptyMessage(String raw) {
        return "没有包含“" + query(raw) + "”的记录";
    }
}
