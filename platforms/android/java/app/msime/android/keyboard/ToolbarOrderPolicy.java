package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** 工具栏的稳定按钮身份；显隐不影响顺序，品牌和收起键不参与排列。 */
public final class ToolbarOrderPolicy {
    public static final int MAX_STORED_CHARS = 512;
    /** 工具栏语音按钮的共享偏好，见 {@link Item#VOICE}。 */
    public static final String VOICE_SHORTCUT_PREFERENCE = "touch_voice_shortcut";

    public enum Item {
        EMOJI("emoji", "表情", "emoji"),
        PHRASE("phrase", "常用语", AndroidLocalSettings.TOOLBAR_PHRASE),
        CLIPBOARD("clipboard", "剪贴板", "clipboard"),
        SKIN("skin", "皮肤", "skin"),
        SCHEME("scheme", "输入方式", AndroidLocalSettings.TOOLBAR_SCHEME),
        FLOATING("floating", "浮动键盘", AndroidLocalSettings.TOOLBAR_FLOATING),
        TEXT_EDIT("text_edit", "文本编辑", AndroidLocalSettings.TOOLBAR_TEXT_EDIT),
        UNDO("undo", "撤销", "undo"),
        REDO("redo", "重做", "redo"),
        /** 语音按钮的开关不在 `touch_toolbar` 里，是共享偏好顶层的 `touch_voice_shortcut`（「键盘 → 工具栏 → 语音」和键盘内「顶部语音入口」写这一项，语音页选「无」时关掉它，#10309），默认关。旧版本存下的顺序里没有它，`parse` 把它补在末尾（#10142）。 */
        VOICE("voice", "语音", VOICE_SHORTCUT_PREFERENCE),
        RANDOM_DIGITS("random_digits", "随机数字", AndroidLocalSettings.TOOLBAR_RANDOM_DIGITS);

        public final String id;
        public final String label;
        public final String preferenceKey;

        Item(String id, String label, String preferenceKey) {
            this.id = id;
            this.label = label;
            this.preferenceKey = preferenceKey;
        }
    }

    public static final List<Item> DEFAULT_ORDER = List.of(Item.values());

    private ToolbarOrderPolicy() {}

    /** 只接受有界文本；未知或重复项忽略，缺少的按钮补在末尾，方便升级后增加按钮。 */
    public static String normalize(String stored) {
        if (stored == null || stored.length() > MAX_STORED_CHARS) return null;
        return format(parse(stored));
    }

    public static List<Item> ordered(String stored) {
        if (stored == null || stored.length() > MAX_STORED_CHARS) return DEFAULT_ORDER;
        return parse(stored);
    }

    private static List<Item> parse(String stored) {
        List<Item> result = new ArrayList<>();
        for (String id : stored.split(",")) {
            for (Item item : DEFAULT_ORDER) {
                if (item.id.equals(id.trim()) && !result.contains(item)) result.add(item);
            }
        }
        for (Item item : DEFAULT_ORDER) if (!result.contains(item)) result.add(item);
        return List.copyOf(result);
    }

    public static String format(List<Item> items) {
        List<String> ids = new ArrayList<>();
        for (Item item : items) ids.add(item.id);
        return String.join(",", ids);
    }

    /** 越界移动保留原顺序，返回的列表始终不可变。 */
    public static List<Item> move(List<Item> order, Item item, int delta) {
        int index = order.indexOf(item);
        int target = index + delta;
        if (index < 0 || target < 0 || target >= order.size()) return List.copyOf(order);
        List<Item> result = new ArrayList<>(order);
        Collections.swap(result, index, target);
        return List.copyOf(result);
    }
}
