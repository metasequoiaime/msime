import app.msime.android.GlideTypingPolicy;

public final class GlideTypingPolicySmoke {
    public static void main(String[] args) {
        check(GlideTypingPolicy.armed(true, true, 0, false, "none", 26), "quanpin on the 26-key letters glides");
        check(!GlideTypingPolicy.armed(false, true, 0, false, "none", 26), "the setting is off by default and then nothing glides");
        check(!GlideTypingPolicy.armed(true, false, 0, false, "none", 26), "nine-key, handwriting and symbol layers do not glide");
        check(!GlideTypingPolicy.armed(true, true, 1, false, "none", 26), "shuangpin letters are not quanpin");
        check(!GlideTypingPolicy.armed(true, true, 0, true, "none", 26), "dedicated English does not glide");
        check(!GlideTypingPolicy.armed(true, true, 0, false, "emoji", 26), "a local mode does not glide");
        check(!GlideTypingPolicy.armed(true, true, 0, false, "none", 25), "a missing letter key does not glide");

        check(!GlideTypingPolicy.starts('q', 'q', 0, 80, 100), "staying on the key is a tap");
        check(!GlideTypingPolicy.starts('q', (char) 0, 0, 80, 100), "the gap between keys is not another key");
        check(!GlideTypingPolicy.starts('t', 'g', 50, 70, 100), "a mostly vertical move is the swipe-for-symbol gesture");
        check(!GlideTypingPolicy.starts('q', 'w', 50, 89.9f, 100), "less than 0.4 key widths across is not yet a glide");
        check(GlideTypingPolicy.starts('q', 'w', 50, 90, 100), "0.4 key widths onto another letter is a glide");
        check(GlideTypingPolicy.starts('w', 'q', 90, 50, 100), "leftwards counts the same");
        check(!GlideTypingPolicy.starts('q', 'w', 0, 100, 0), "an unmeasured keyboard never glides");

        float[] centers = new float[52];
        for (int index = 0; index < 52; index++) centers[index] = index * 1.5f;
        float[] xs = {10, 20.5f, 30};
        float[] ys = {5, 6, 7};
        long[] times = {0, 8, 16};
        String request = GlideTypingPolicy.request(centers, 80, 120, xs, ys, times, 3);
        check(request.startsWith("{\"keys\":[[0.0,1.5],[3.0,4.5],"), request);
        check(request.contains("\"key_width\":80.0,\"key_height\":120.0"), request);
        check(request.endsWith("\"points\":[[10.0,5.0,0],[20.5,6.0,8],[30.0,7.0,16]]}"), request);
        check(occurrences(request, "],[") == 25 + 2, "26 keys and 3 points: " + request);

        int count = 5000;
        float[] longX = new float[count];
        float[] longY = new float[count];
        long[] longT = new long[count];
        for (int index = 0; index < count; index++) {
            longX[index] = index;
            longY[index] = 1;
            longT[index] = index;
        }
        String capped = GlideTypingPolicy.request(centers, 80, 120, longX, longY, longT, count);
        String points = capped.substring(capped.indexOf("\"points\":"));
        check(occurrences(points, "],[") == GlideTypingPolicy.MAX_POINTS - 1, "a long stroke is capped at 1024 points");
        check(points.startsWith("\"points\":[[0.0,1.0,0]"), "the first point is kept");
        check(points.endsWith("[4999.0,1.0,4999]]}"), "the last point is kept");

        expectFailure(() -> GlideTypingPolicy.request(new float[50], 80, 120, xs, ys, times, 3), "fewer than 26 keys");
        expectFailure(() -> GlideTypingPolicy.request(centers, 80, 120, xs, ys, times, 1), "a single point");
        expectFailure(() -> GlideTypingPolicy.request(centers, Float.NaN, 120, xs, ys, times, 3), "a non-finite key size");
        System.out.println("Android glide typing: arming, the 0.4-key start, request JSON and the 1024-point cap passed");
    }

    private static int occurrences(String text, String needle) {
        int found = 0;
        for (int at = text.indexOf(needle); at >= 0; at = text.indexOf(needle, at + 1)) found++;
        return found;
    }

    private static void expectFailure(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError("expected a failure for " + message);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
