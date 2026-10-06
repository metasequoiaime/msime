package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 功能面板（点工具栏品牌键打开）的条目与分页，按计划 P25：每页 4 列 × 2 行，AI 回复和 AI 润色合成一格「AI 回复与润色」，腾出一格给本地输入。
 *
 * <p>只描述「有什么、叫什么、是不是开关」；开关当前值和点击后的动作由 IME 根据 {@link Item#id()} 决定。
 */
public final class FunctionPanelModel {
    public static final int COLUMNS = 4;
    public static final int ROWS = 2;
    public static final int PAGE_SIZE = COLUMNS * ROWS;

    /** 条目标识，IME 按它接线。 */
    public enum Id {
        FULL_WIDTH, CHINESE_PUNCTUATION, FUZZY_PINYIN, TRADITIONAL, HANDWRITING, DICTIONARY,
        KEYBOARD_HEIGHT, SETTINGS,
        KEY_SOUND, VIBRATION, ONE_HAND, PRIVACY, FEEDBACK, ABOUT, AI_ASSIST, LOCAL_INPUT,
        VOICE_RESULT, VIBRATION_STRENGTH, EMOJI, CLIPBOARD
    }

    /**
     * 一个条目。
     *
     * @param id 标识
     * @param label 条目上画的文字（无障碍节点 text）
     * @param description contentDescription（§2.8：多数与标签相同，繁体为「繁体输出」、振动为「按键振动」）
     * @param toggle 是否是开关（开关条目带 stateDescription 已开启 / 已关闭，开启时画强调色和 ✓ 角标）
     */
    public record Item(Id id, String label, String description, boolean toggle) {}

    private static final List<Item> ITEMS = List.of(
        toggle(Id.FULL_WIDTH, "全角", "全角"),
        toggle(Id.CHINESE_PUNCTUATION, "中文标点", "中文标点"),
        toggle(Id.FUZZY_PINYIN, "模糊音", "模糊音"),
        toggle(Id.TRADITIONAL, "繁体", "繁体输出"),
        action(Id.HANDWRITING, "手写"),
        action(Id.DICTIONARY, "词库"),
        action(Id.KEYBOARD_HEIGHT, "键盘高度"),
        action(Id.SETTINGS, "设置"),
        toggle(Id.KEY_SOUND, "按键音", "按键音"),
        toggle(Id.VIBRATION, "振动", "按键振动"),
        toggle(Id.ONE_HAND, "单手模式", "单手模式"),
        toggle(Id.PRIVACY, "隐私模式", "隐私模式"),
        action(Id.FEEDBACK, "反馈"),
        action(Id.ABOUT, "关于"),
        action(Id.AI_ASSIST, "AI 回复与润色"),
        action(Id.LOCAL_INPUT, "本地输入"),
        action(Id.VOICE_RESULT, "语音结果"),
        action(Id.VIBRATION_STRENGTH, "振动强度"),
        action(Id.EMOJI, "表情"),
        action(Id.CLIPBOARD, "剪贴板历史"));

    private FunctionPanelModel() { }

    private static Item toggle(Id id, String label, String description) {
        return new Item(id, label, description, true);
    }

    private static Item action(Id id, String label) {
        return new Item(id, label, label, false);
    }

    /** 全部条目，按页序排列。 */
    public static List<Item> items() { return ITEMS; }

    public static int pageCount() {
        return (ITEMS.size() + PAGE_SIZE - 1) / PAGE_SIZE;
    }

    /** 第 `page` 页（从 0 起）的条目；越界时为空列表。 */
    public static List<Item> page(int page) {
        if (page < 0 || page >= pageCount()) return List.of();
        int from = page * PAGE_SIZE;
        return ITEMS.subList(from, BoundsPolicy.atMost(ITEMS.size(), from + PAGE_SIZE));
    }

    /** 某个条目所在的页（从 0 起）。 */
    public static int pageOf(Id id) {
        return indexOf(id) / PAGE_SIZE;
    }

    public static Item item(Id id) {
        return ITEMS.get(indexOf(id));
    }

    /** 全部开关条目。 */
    public static List<Item> toggles() {
        List<Item> result = new ArrayList<>(ITEMS.size());
        for (Item item : ITEMS) if (item.toggle()) result.add(item);
        return List.copyOf(result);
    }

    /** 条目的 stateDescription：不可用时「不可用」，开关按值「已开启 / 已关闭」，非开关条目为 null。 */
    public static String state(Item item, boolean on, boolean enabled) {
        if (!enabled) return "不可用";
        if (!item.toggle()) return null;
        return on ? "已开启" : "已关闭";
    }

    private static int indexOf(Id id) {
        for (int index = 0; index < ITEMS.size(); index++)
            if (ITEMS.get(index).id() == id) return index;
        throw new IllegalArgumentException("Unknown function panel item " + id);
    }
}
