import app.msime.android.KoreanInputPolicy;
import app.msime.android.StrokeInputPolicy;
import app.msime.android.ZhuyinInputPolicy;

/** 笔画的内联组字是 reading 里的笔画字形，不是 editing_text 里的字母。 */
public final class StrokeInputPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(StrokeInputPolicy.STROKE_SCHEME == 8, "the shared Engine ordinal for Stroke is 8");
        check(StrokeInputPolicy.active(8, false), "Stroke outside dedicated English takes the keys");
        check(!StrokeInputPolicy.active(8, true), "dedicated English is the host's own");
        check(!StrokeInputPolicy.active(5, false) && !StrokeInputPolicy.active(6, false)
            && !StrokeInputPolicy.active(0, false) && !StrokeInputPolicy.active(9, false), "only scheme 8");

        // The same rule the service applies to a transition: a scheme that draws its reading marks the reading.
        boolean marksReading = KoreanInputPolicy.active(8, false) || ZhuyinInputPolicy.active(8, false)
            || StrokeInputPolicy.active(8, false);
        check(marksReading, "a Stroke view marks its reading");
        check("一丨＊".equals(KoreanInputPolicy.composing(marksReading, "", "hsx", "一丨＊")),
            "the editor shows the stroke glyphs, never the letters");
        check("".equals(KoreanInputPolicy.composing(marksReading, "", "", "")),
            "nothing is marked once the composition is gone");
        System.out.println("Android stroke input policy: activity and the inline reading passed");
    }
}
