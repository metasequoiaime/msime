package app.msime.android;

/** 工具栏右侧常驻按钮的取值规则；按钮身份复用 {@link ToolbarOrderPolicy}。 */
public final class ToolbarPinnedPolicy {
    public static final ToolbarOrderPolicy.Item DEFAULT = ToolbarOrderPolicy.Item.DISMISS;

    private ToolbarPinnedPolicy() {}

    public static String[] choices() {
        ToolbarOrderPolicy.Item[] items = ToolbarOrderPolicy.Item.values();
        String[] ids = new String[items.length];
        for (int index = 0; index < items.length; index++) ids[index] = items[index].id;
        return ids;
    }

    public static ToolbarOrderPolicy.Item item(String value) {
        if (value != null) {
            for (ToolbarOrderPolicy.Item item : ToolbarOrderPolicy.Item.values())
                if (item.id.equals(value)) return item;
        }
        return DEFAULT;
    }

    public static String label(String value) {
        return item(value).label;
    }
}
