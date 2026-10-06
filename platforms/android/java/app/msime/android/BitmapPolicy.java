package app.msime.android;

import android.graphics.Bitmap;

/** Shared bitmap geometry for host image-processing flows. */
public final class BitmapPolicy {
    private BitmapPolicy() {}

    /** Return the longer of two bitmap dimensions. */
    public static int longestEdge(int width, int height) {
        return Math.max(width, height);
    }

    /** Return a bitmap's longer edge, or zero for a null bitmap. */
    public static int longestEdge(Bitmap bitmap) {
        return bitmap == null ? 0 : longestEdge(bitmap.getWidth(), bitmap.getHeight());
    }

    /** Scale a bitmap down proportionally when its longest edge exceeds the requested limit. */
    public static Bitmap scaleToEdge(Bitmap bitmap, int maximumEdge) {
        if (bitmap == null || maximumEdge <= 0) return bitmap;
        int edge = longestEdge(bitmap);
        if (edge <= maximumEdge) return bitmap;
        float scale = maximumEdge / (float) edge;
        Bitmap scaled = Bitmap.createScaledBitmap(bitmap,
            Math.max(1, Math.round(bitmap.getWidth() * scale)),
            Math.max(1, Math.round(bitmap.getHeight() * scale)), true);
        if (scaled != bitmap) bitmap.recycle();
        return scaled;
    }

    /** Return the largest power-of-two decoder sample that keeps the longest edge useful. */
    public static int sampleSizeForEdge(int width, int height, int maximumEdge) {
        if (width <= 0 || height <= 0 || maximumEdge <= 0) return 1;
        int sample = 1;
        int edge = longestEdge(width, height);
        while (edge / (sample * 2) >= maximumEdge) sample *= 2;
        return sample;
    }
}
