import app.msime.android.ClipboardSegmentation;
import app.msime.android.ClipboardSegmentation.Segment;
import java.text.BreakIterator;
import java.util.List;
import java.util.Locale;

/** #5645：剪贴板分词切出的片段首尾相接就是原文，拼回时只保留夹在相邻已选词片之间的空白。 */
public final class ClipboardSegmentationSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static List<Segment> segment(String text) {
        return ClipboardSegmentation.segment(text, BreakIterator.getWordInstance(Locale.CHINESE));
    }

    private static String concatenated(List<Segment> segments) {
        StringBuilder out = new StringBuilder();
        for (Segment segment : segments) out.append(segment.text());
        return out.toString();
    }

    private static boolean[] select(List<Segment> segments, String... words) {
        boolean[] selected = new boolean[segments.size()];
        for (int index = 0; index < segments.size(); index++)
            for (String word : words) if (segments.get(index).text().equals(word)) selected[index] = true;
        return selected;
    }

    public static void main(String[] args) {
        String sample = "Android 11, vivo x60  os1.0\n客户端版本：0.2.2";
        List<Segment> segments = segment(sample);
        check(concatenated(segments).equals(sample), "segments cover the text in order, nothing lost");
        for (int index = 1; index < segments.size(); index++)
            check(!(segments.get(index).separator() && segments.get(index - 1).separator()),
                "adjacent whitespace merges into one separator");
        for (Segment segment : segments)
            if (segment.separator()) check(segment.text().isBlank(), "only whitespace separates");
        check(ClipboardSegmentation.selectableCount(segments) > 5, "the sample splits into several pickable pieces");

        // 掐头去尾：选中间连续的几个词，空白跟着保留，两头的不要。
        List<Segment> english = segment("please copy only this part, thanks");
        check("only this part".equals(ClipboardSegmentation.join(english, select(english, "only", "this", "part"))),
            "adjacent picked words keep the spaces between them");
        check("onlypart".equals(ClipboardSegmentation.join(english, select(english, "only", "part"))),
            "a skipped word drops the spaces around it");
        check("".equals(ClipboardSegmentation.join(english, new boolean[english.size()])),
            "nothing picked joins to nothing");
        boolean[] all = new boolean[english.size()];
        java.util.Arrays.fill(all, true);
        check("please copy only this part, thanks".equals(ClipboardSegmentation.join(english, all)),
            "picking everything gives the original text back");

        // 长串汉字（词典里没有、或实现不按词切）拆成单字，用户仍能逐字掐头去尾。
        List<Segment> chinese = segment("我们今天去北京看看长城");
        check(concatenated(chinese).equals("我们今天去北京看看长城"), "Chinese segments cover the text");
        for (Segment segment : chinese)
            check(segment.text().codePointCount(0, segment.text().length())
                    <= ClipboardSegmentation.MAX_IDEOGRAPH_RUN,
                "no Chinese piece is longer than the run limit");
        check("北京".equals(ClipboardSegmentation.join(chinese, select(chinese, "北京", "北", "京"))),
            "picked Chinese pieces join without separators");

        // 代理对不从中间切开；超长文字只切开头。
        String emoji = "🌲".repeat(ClipboardSegmentation.MAX_CHARS);
        check(ClipboardSegmentation.truncated(emoji), "over-long text is truncated");
        String head = concatenated(segment(emoji));
        check(head.length() <= ClipboardSegmentation.MAX_CHARS, "only the head is segmented");
        check(!Character.isHighSurrogate(head.charAt(head.length() - 1)), "a surrogate pair is never split");
        check(segment("").isEmpty() && segment(null).isEmpty(), "no text, no segments");
        try {
            ClipboardSegmentation.join(english, new boolean[1]);
            throw new AssertionError("a mismatched selection is refused");
        } catch (IllegalArgumentException expected) {
            // 选中状态必须与片段一一对应。
        }
        System.out.println("ClipboardSegmentationSmoke ok");
    }
}
