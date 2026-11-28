package app.msime.android.home;

import java.util.ArrayList;
import java.util.List;

/**
 * 「表达 › 标点 › 保留英文的标点」的规则（#11890）：共享偏好 `preserve_english_punctuation` 记一组键，这些键在中文标点下照样输出英文标点。可选的键是标点契约第 7 版 `preserveEnglish` 段的 21 个，按契约顺序；每项旁边画出这个键在中文标点下原本出的字。读写偏好的那一半在 `ExpressionPage` 里，这里只有纯规则，JVM 冒烟测试可以直接跑。
 */
public final class PreserveEnglishPunctuation {
    public static final String KEY = "preserve_english_punctuation";

    /** 契约 `preserveEnglish` 段的键与它们在中文标点下原本出的字，按契约顺序。 */
    public record Key(String ascii, String chinese) {}

    public static final List<Key> KEYS = List.of(
        new Key(",", "，"), new Key(".", "。"), new Key("?", "？"), new Key("!", "！"), new Key(";", "；"),
        new Key(":", "："), new Key("(", "（"), new Key(")", "）"), new Key("[", "【"), new Key("]", "】"),
        new Key("\\", "、"), new Key("`", "·"), new Key("$", "￥"), new Key("^", "……"), new Key("_", "——"),
        new Key("{", "「"), new Key("}", "」"), new Key("\"", "“ ”"), new Key("'", "‘ ’"), new Key("<", "《"),
        new Key(">", "》"));

    private PreserveEnglishPunctuation() {}

    /** 文档里的集合，只留认识的键，按契约顺序，不重复。 */
    public static List<String> selected(List<String> stored) {
        List<String> keys = new ArrayList<>();
        for (Key key : KEYS) if (stored.contains(key.ascii())) keys.add(key.ascii());
        return keys;
    }

    /** 点了一个键之后的集合：选着的去掉，没选的加上，仍按契约顺序。 */
    public static List<String> toggled(List<String> stored, String ascii) {
        List<String> next = new ArrayList<>(selected(stored));
        if (!next.remove(ascii)) next.add(ascii);
        return selected(next);
    }

    /** 行尾显示：空集合是「无」，否则按契约顺序列出键。 */
    public static String summary(List<String> stored) {
        List<String> keys = selected(stored);
        return keys.isEmpty() ? "无" : String.join(" ", keys);
    }

    /** 选项文字：键本身和它原本出的字。 */
    public static String label(Key key) {
        return key.ascii() + "    " + key.chinese();
    }
}
