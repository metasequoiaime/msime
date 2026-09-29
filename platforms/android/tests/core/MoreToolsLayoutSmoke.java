import app.msime.android.MoreToolsLayout;

public final class MoreToolsLayoutSmoke {
    public static void main(String[] args) {
        check(MoreToolsLayout.Section.TOOLS.columns() == 4, "tools use four-column tiles");
        check(MoreToolsLayout.Section.SETTINGS.columns() == 4, "settings use four-column tiles");
        check(MoreToolsLayout.Section.TOOLS.tiles() && MoreToolsLayout.Section.SETTINGS.tiles(),
            "tools and settings are tiles");
        check(!MoreToolsLayout.Section.LOCAL_INPUT.tiles()
            && !MoreToolsLayout.Section.LOCAL_INPUT_BACK.tiles(), "local input stays a card list");
        check(MoreToolsLayout.Section.TOOLS.height() == 52
            && MoreToolsLayout.Section.LOCAL_INPUT.height() == 48, "tile and card heights");
        check(MoreToolsLayout.TILE_RADIUS_DP == 16, "tile radius");
        check(MoreToolsLayout.Section.LOCAL_INPUT.columns() == 2,
            "local input uses two columns");
        check(MoreToolsLayout.Section.LOCAL_INPUT_BACK.columns() == 1,
            "local input navigation uses one full-width column");
        check(MoreToolsLayout.rowCount(6, MoreToolsLayout.Section.TOOLS) == 2,
            "six tools occupy two tile rows");
        check(MoreToolsLayout.rowCount(8, MoreToolsLayout.Section.LOCAL_INPUT) == 4,
            "eight local tools occupy four rows");
        check("已开启".equals(MoreToolsLayout.state(MoreToolsLayout.Section.SETTINGS, true)),
            "enabled setting state");
        check("已关闭".equals(MoreToolsLayout.state(MoreToolsLayout.Section.SETTINGS, false)),
            "disabled setting state");
        check("☺".equals(MoreToolsLayout.icon("表情")), "emoji icon");
        check("♪".equals(MoreToolsLayout.icon("按键音")), "sound icon");
        check("⚙".equals(MoreToolsLayout.icon("应用设置")), "client app entry icon");
        check("⌨".equals(MoreToolsLayout.icon("未知工具")), "fallback icon");
        check(MoreToolsLayout.CARD_HEIGHT_DP == 48 && MoreToolsLayout.HEADER_HEIGHT_DP == 44,
            "card and header dimensions");
        boolean rejected = false;
        try { MoreToolsLayout.rowCount(-1, MoreToolsLayout.Section.TOOLS); }
        catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "negative counts rejected");
        System.out.println("Android more tools: tile grouping, dimensions and states passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
