package app.msime.android;

/** 右侧常驻按钮的缺省、非法值回退和稳定 ID 映射。 */
public final class ToolbarPinnedPolicySmoke {
    public static void main(String[] args) {
        check(ToolbarPinnedPolicy.item(null) == ToolbarOrderPolicy.Item.DISMISS, "missing pin defaults to dismiss");
        check(ToolbarPinnedPolicy.item("unknown") == ToolbarOrderPolicy.Item.DISMISS, "unknown pin defaults to dismiss");
        check(ToolbarPinnedPolicy.item("text_edit") == ToolbarOrderPolicy.Item.TEXT_EDIT, "text edit pin maps by ID");
        check("文本编辑".equals(ToolbarPinnedPolicy.label("text_edit")), "pin label follows item");
        check(ToolbarPinnedPolicy.choices().length == ToolbarOrderPolicy.DEFAULT_ORDER.size(), "pin choices cover toolbar items");
        check(AndroidLocalSettings.defaults().choice(AndroidLocalSettings.TOOLBAR_PINNED)
            .equals(ToolbarPinnedPolicy.DEFAULT.id), "registered pin default matches policy");
        check(AndroidLocalSettings.defaults().bool(AndroidLocalSettings.TOOLBAR_DISMISS), "dismiss toolbar item defaults on");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.TOOLBAR_PINNED).synced,
            "pin stays local");
        System.out.println("ToolbarPinnedPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
