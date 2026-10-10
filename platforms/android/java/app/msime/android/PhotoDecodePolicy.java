package app.msime.android;

/** 自定义皮肤照片的解码尺寸上限，避免压缩图片以超大像素图进入键盘进程。 */
public final class PhotoDecodePolicy {
    public static final int MAX_DECODE_EDGE = 2048;

    private PhotoDecodePolicy() {}

    /** 返回让解码结果长边不超过上限的二次幂采样值；尺寸无效时返回零。 */
    public static int sampleSize(int width, int height) {
        if (width <= 0 || height <= 0) return 0;
        long edge = maxEdge(width, height);
        long sample = 1;
        while (edge > (long) MAX_DECODE_EDGE * sample) {
            if (sample > Integer.MAX_VALUE / 2L) return 0;
            sample <<= 1;
        }
        return (int) sample;
    }

    private static int maxEdge(int width, int height) {
        return BoundsPolicy.atLeast(width, height);
    }

    /** BitmapFactory 仍返回原尺寸时拒绝它，避免采样策略失效后继续持有大图。 */
    public static boolean withinBounds(int width, int height) {
        return width > 0 && height > 0
            && maxEdge(width, height) <= MAX_DECODE_EDGE;
    }
}
