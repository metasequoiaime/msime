import app.msime.android.TextTrackpadSurface;

public final class TextTrackpadSurfaceSmoke {
    public static void main(String[] args) {
        // 点阵在边上居中：两端留的空相等，最后一个点不越界。
        for (float length : new float[] {16, 31, 100, 333.5f, 1080}) {
            int count = TextTrackpadSurface.dotCount(length, 16);
            float first = TextTrackpadSurface.firstDot(length, 16);
            float last = first + (count - 1) * 16;
            check(count >= 1 && first > 0 && last < length, "dots fit: " + length);
            check(Math.abs(first - (length - last)) < .001f, "dots centred: " + length);
        }
        check(TextTrackpadSurface.dotCount(15, 16) == 0 && TextTrackpadSurface.firstDot(15, 16) == 0,
            "too short for a dot");
        check(TextTrackpadSurface.dotCount(100, 0) == 0 && TextTrackpadSurface.dotCount(Float.NaN, 16) == 0,
            "bad spacing or length draws nothing");

        // 提示文字：放得下用上限字号，放不下等比缩小，缩到下限以下或高度不够一行就不画。
        check(TextTrackpadSurface.hintSize(80, 200, 20, 100, 14, 10) == 14, "fits at max size");
        float shrunk = TextTrackpadSurface.hintSize(160, 120, 20, 100, 14, 10);
        check(Math.abs(shrunk - 10.5f) < .001f, "shrinks to fit: " + shrunk);
        check(TextTrackpadSurface.hintSize(160, 100, 20, 100, 14, 10) == 0, "below min size is hidden");
        check(TextTrackpadSurface.hintSize(80, 200, 20, 19, 14, 10) == 0, "too short for a line is hidden");
        check(TextTrackpadSurface.hintSize(80, -4, 20, 100, 14, 10) == 0, "no width is hidden");
        System.out.println("Android text trackpad surface: centred dot grid and fitted hint passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
