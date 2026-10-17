package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** 符号面板分类的稳定身份与本机排列规则。 */
public final class SymbolCategoryOrderPolicy {
    public static final int MAX_STORED_CHARS = 512;

    public enum Item {
        RECENTS("recents", "常用"),
        CHINESE("chinese", "中文"),
        ENGLISH("english", "英文"),
        NUMBERS("numbers", "数字"),
        NETWORK("network", "网络"),
        PINYIN("pinyin", "拼音"),
        KAOMOJI("kaomoji", "颜文字"),
        SHAPES("shapes", "形状"),
        ARROWS("arrows", "箭头"),
        PUNCTUATION("punctuation", "标点"),
        MATH("math", "数学"),
        CURRENCY("currency", "货币"),
        HEARTS("hearts", "爱心"),
        LETTERS("letters", "字母"),
        GAMES("games", "游戏"),
        CULTURE("culture", "文化"),
        NATURE("nature", "自然"),
        PEOPLE("people", "人物"),
        MORE("more", "更多");

        public final String id;
        public final String label;

        Item(String id, String label) {
            this.id = id;
            this.label = label;
        }
    }

    public static final List<Item> DEFAULT_ORDER = List.of(Item.values());

    private SymbolCategoryOrderPolicy() {}

    /** 只接受有界文本；未知或重复项忽略，缺少的分类补在末尾。 */
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
