import app.msime.android.HostDeepLink;
import app.msime.android.home.SettingsSearchAnchorPolicy;
import java.util.List;

/**
 * 设置首页搜索结果要带着命中的设置项打开详情页：以前结果和首页普通入口一样只给页面，带分类标签的页面总是停在第一个分类，搜「显示页码」打开「键盘」停在「布局」，这一行在「工具栏」。
 *
 * <p>这里钉住不碰 Android 视图的那一半：结果带哪个关键词作为目标、这个关键词什么时候直接就是一个分类、页面上的哪段文字算命中、参数怎么写进 Bundle 的键。在视图树里找这一行、切分类和滚动由 SettingsSearchAnchorDeviceChecks 点击真实搜索结果验证；短暂高亮的观感仍需设备验收。
 */
public final class SettingsSearchAnchorPolicySmoke {
    private static final List<String> KEYBOARD_SECTIONS = List.of("布局", "反馈", "手势", "工具栏");

    public static void main(String[] arguments) {
        anchor();
        sections();
        labels();
        argument();
        System.out.println("SettingsSearchAnchorPolicySmoke ok");
    }

    private static void anchor() {
        List<String> keywords = List.of("布局", "按钮顺序", "键盘工具栏", "显示方式", "候选栏大小", "显示页码", "工具栏尺寸");
        check("显示页码".equals(SettingsSearchAnchorPolicy.anchor(keywords, "显示页码")), "the exact keyword is the anchor");
        check("显示页码".equals(SettingsSearchAnchorPolicy.anchor(keywords, "页码")), "a partial query anchors the first keyword containing it");
        check("显示页码".equals(SettingsSearchAnchorPolicy.anchor(keywords, "  页码 ")), "the query is trimmed like PageId#matches");
        check("键盘工具栏".equals(SettingsSearchAnchorPolicy.anchor(keywords, "工具栏")), "keywords are tried in their declared order");
        check(SettingsSearchAnchorPolicy.anchor(keywords, "浮动") == null, "a query no keyword contains has no anchor, the page opens as from the home row");
        check(SettingsSearchAnchorPolicy.anchor(keywords, "") == null, "an empty query has no anchor");
        check(SettingsSearchAnchorPolicy.anchor(keywords, "   ") == null, "a blank query has no anchor");
        check("组字时保留 Shift".equals(SettingsSearchAnchorPolicy.anchor(List.of("组字时保留 Shift", "Shift"), "SHIFT")),
            "matching ignores case like PageId#matches and returns the keyword in its own spelling");
    }

    private static void sections() {
        check("工具栏".equals(SettingsSearchAnchorPolicy.section(KEYBOARD_SECTIONS, "工具栏")), "an anchor naming a section opens that section");
        check(SettingsSearchAnchorPolicy.section(KEYBOARD_SECTIONS, "显示页码") == null, "a row anchor is not a section; the page looks for the row");
        check("社区插件".equals(SettingsSearchAnchorPolicy.section(List.of("我的插件", "社区插件"), "社区插件")),
            "searching 社区插件 opens the plugins page on its community section");
        check(SettingsSearchAnchorPolicy.section(List.of(), "工具栏") == null, "a page without sections has no section to open");
        check(SettingsSearchAnchorPolicy.section(KEYBOARD_SECTIONS, null) == null, "no anchor, no section");
    }

    private static void labels() {
        check(SettingsSearchAnchorPolicy.labels("显示页码", "显示页码"), "a row title equal to the anchor is the row");
        check(SettingsSearchAnchorPolicy.labels(" 显示页码\n", "显示页码"), "surrounding whitespace does not matter");
        check(!SettingsSearchAnchorPolicy.labels("显示页码与读音", "显示页码"), "a longer title is a different row");
        check(!SettingsSearchAnchorPolicy.labels("页码", "显示页码"), "a shorter title is a different row");
        check(!SettingsSearchAnchorPolicy.labels(null, "显示页码"), "a view without text is not the row");
        check(!SettingsSearchAnchorPolicy.labels("", ""), "an empty anchor never matches");
    }

    private static void argument() {
        check("row".equals(HostDeepLink.ARG_ROW), "the anchor travels under the row argument");
        check(HostDeepLink.isAllowedArgKey(HostDeepLink.ARG_ROW), "the anchor key passes the deep link's key filter");
        check(!HostDeepLink.ARG_ROW.equals(HostDeepLink.ARG_SECTION), "row and section are separate arguments");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
