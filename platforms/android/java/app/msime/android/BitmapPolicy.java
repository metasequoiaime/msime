package app.msime.android;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;

/** Shared bitmap geometry for host image-processing flows. */
public final class BitmapPolicy {
    private BitmapPolicy() {}

    /** Return the longer of two bitmap dimensions. */
    public static int longestEdge(int width, int height) {
        return BoundsPolicy.atLeast(width, height);
    }

    /** Return a bitmap's longer edge, or zero for a null bitmap. */
    public static int longestEdge(Bitmap bitmap) {
        return bitmap == null ? 0 : longestEdge(bitmap.getWidth(), bitmap.getHeight());
    }

    /** Decode only image dimensions from an in-memory payload, or return null when invalid. */
    public static BitmapFactory.Options decodeBounds(byte[] bytes) {
        if (bytes == null || bytes.length == 0) return null;
        BitmapFactory.Options bounds = new BitmapFactory.Options();
        bounds.inJustDecodeBounds = true;
        BitmapFactory.decodeByteArray(bytes, 0, bytes.length, bounds);
        return bounds.outWidth <= 0 || bounds.outHeight <= 0 ? null : bounds;
    }

    /** Decode only image dimensions from a stream, or return null when invalid. */
    public static BitmapFactory.Options decodeBounds(InputStream input) throws IOException {
        if (input == null) return null;
        BitmapFactory.Options bounds = new BitmapFactory.Options();
        bounds.inJustDecodeBounds = true;
        BitmapFactory.decodeStream(input, null, bounds);
        return bounds.outWidth <= 0 || bounds.outHeight <= 0 ? null : bounds;
    }

    /** Scale a bitmap down proportionally when its longest edge exceeds the requested limit. */
    public static Bitmap scaleToEdge(Bitmap bitmap, int maximumEdge) {
        if (bitmap == null || maximumEdge <= 0) return bitmap;
        int edge = longestEdge(bitmap);
        if (edge <= maximumEdge) return bitmap;
        float scale = maximumEdge / (float) edge;
        Bitmap scaled = Bitmap.createScaledBitmap(bitmap,
            BoundsPolicy.atLeast(Math.round(bitmap.getWidth() * scale), 1),
            BoundsPolicy.atLeast(Math.round(bitmap.getHeight() * scale), 1), true);
        if (scaled != bitmap) bitmap.recycle();
        return scaled;
    }

    /** Encode a bitmap as JPEG using the shared quality ladder until it fits the byte limit. */
    public static byte[] compressJpegUnderBytes(Bitmap bitmap, int maximumBytes) {
        if (bitmap == null || maximumBytes <= 0) return null;
        for (int quality = 85; quality >= 40; quality -= 15) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            if (bitmap.compress(Bitmap.CompressFormat.JPEG, quality, output)
                    && output.size() <= maximumBytes) return output.toByteArray();
        }
        return null;
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
