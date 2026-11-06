package app.msime.android.home;

import app.msime.android.TextPolicy;
import java.util.List;

/**
 * 设置首页搜索结果带着命中的设置项打开详情页时，不碰 Android 视图的那几条规则。
 *
 * <p>结果的目标是命中的关键词（{@link #target}），经 `HostDeepLink.ARG_ROW` 交给页面。关键词本身就是页面的某个分类时（插件页的「社区插件」），页面直接打开那个分类（{@link #section}）；否则 `DetailPage` 在各分类里找标题等于它的一行（{@link #labels}），切到那个分类、滚过去并短暂高亮。只命中页面标题、没有关键词命中时不带目标，页面和从首页那一行进来一样打开。
 *
 * <p>本类只用 Java 标准库和 {@link TextPolicy}，check-host 直接编译并在 JVM 里由 `SettingsSearchAnchorPolicySmoke` 测它。
 */
public final class SettingsSearchAnchorPolicy {
    private SettingsSearchAnchorPolicy() {}

    /** 第一个包含查询（去掉首尾空白、忽略大小写，与 `PageId#matches` 相同）的关键词，原样返回；没有或查询为空时为 null。搜索结果的副标题和打开时定位的目标都用它。 */
    public static String anchor(List<String> keywords, String query) {
        String needle = TextPolicy.lowercaseTrimmed(query);
        if (needle.isEmpty()) return null;
        for (String keyword : keywords) {
            if (TextPolicy.lowercase(keyword).contains(needle)) return keyword;
        }
        return null;
    }

    /** 搜索结果打开页面时带的目标：查询命中了结果标题（例如搜「键盘」）时为 null，用户找的是这一页，页面和从首页那一行进来一样打开，不能被带到碰巧含这个词的一行（键盘页的「中文键盘」）；否则是 {@link #anchor}。 */
    public static String target(String title, List<String> keywords, String query) {
        String needle = TextPolicy.lowercaseTrimmed(query);
        if (needle.isEmpty() || TextPolicy.lowercase(title).contains(needle)) return null;
        return anchor(keywords, query);
    }

    /** 目标就是页面的某个分类标题时返回这个分类，否则为 null。 */
    public static String section(List<String> sections, String anchor) {
        if (anchor == null) return null;
        String wanted = anchor.trim();
        for (String section : sections) {
            if (section.equals(wanted)) return section;
        }
        return null;
    }

    /** 页面上的一段文字是不是目标那一行的标题：去掉首尾空白后完全相同。只是包含不算，「显示页码」不能落在一行更长的标题上。 */
    public static boolean labels(CharSequence text, String anchor) {
        if (text == null || anchor == null) return false;
        String wanted = anchor.trim();
        return !wanted.isEmpty() && text.toString().trim().equals(wanted);
    }
}
