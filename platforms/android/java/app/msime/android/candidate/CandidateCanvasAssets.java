package app.msime.android;

import android.animation.ValueAnimator;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.ImageDecoder;
import android.graphics.Rect;
import android.graphics.drawable.AnimatedImageDrawable;
import android.graphics.drawable.BitmapDrawable;
import android.graphics.drawable.Drawable;
import android.os.Handler;
import android.os.Looper;
import android.view.View;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.Map;
import java.util.WeakHashMap;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 偏好线程准备的画布；绘制路径不读取文件，也不重新解码。 */
public final class CandidateCanvasAssets {
    final Image panel;
    final Tiles panelTiles;
    final double[] slices;
    final double[] widths;
    final boolean fill;
    final double[] content;
    final double[] outer;
    final double lineGap;
    final Double itemRadius;
    final java.util.List<Widget> widgets;
    final String key;

    private CandidateCanvasAssets(JSONObject manifest, Image panel, java.util.List<Widget> widgets,
            String key) {
        this.panel = panel;
        this.widgets = java.util.List.copyOf(widgets);
        this.key = key;
        JSONObject border = manifest.optJSONObject("borderImage");
        slices = border == null ? new double[4] : sides(border.optJSONArray("stretchDip"),
            sides(border.optJSONArray("sliceDip"), new double[4]));
        widths = border == null ? new double[4] : sides(border.optJSONArray("widthDip"), slices);
        panelTiles = panel == null ? null : panel.tiles(slices);
        fill = border == null || border.optBoolean("fill", true);
        content = insets(manifest.optJSONObject("contentInsetsDip"));
        if (panel != null) for (int i = 0; i < 4; i++) content[i] = Math.max(content[i], widths[i]);
        outer = insets(manifest.optJSONObject("outerInsetsDip"));
        lineGap = manifest.optDouble("lineGapDip", -1);
        itemRadius = manifest.has("itemCornerRadiusDip") ? manifest.optDouble("itemCornerRadiusDip") : null;
    }

    public String key() { return key; }
    public boolean panelDrawn() { return panel != null; }

    /** 两个明暗槽共享同一读取批次的资源缓存；上次内容相同的位图也继续复用。 */
    static CandidateCanvasAssets load(String directory, String id, boolean dark,
            Map<String, Image> cache, Map<String, Image> previous) throws JSONException, IOException {
        JSONObject query = new JSONObject().put("directory", directory).put("id", id);
        JSONObject manifest = value(NativeClient.skinPackage(query.toString()));
        if (manifest == null || !manifest.optBoolean("canvas")) return null;
        StringBuilder fingerprint = new StringBuilder(manifest.toString()).append(dark);
        Map<String, Image> loaded = new HashMap<>();
        java.util.function.Function<String, Image> image = relative -> {
            if (relative == null || relative.isEmpty()) return null;
            if (loaded.containsKey(relative)) return loaded.get(relative);
            Image result = null;
            try {
                JSONObject resource = value(NativeClient.skinResource(new JSONObject(query.toString())
                    .put("relative", relative).put("kind", "image").toString()));
                JSONArray raw = resource == null ? null : resource.optJSONArray("bytes");
                if (raw == null || raw.length() == 0 || raw.length() > 1_048_576) throw new IOException();
                byte[] bytes = new byte[raw.length()];
                for (int i = 0; i < bytes.length; i++) bytes[i] = (byte) raw.getInt(i);
                String digest = digest(bytes);
                fingerprint.append(relative).append(digest);
                result = cache.get(digest);
                if (result == null) result = previous.get(digest);
                if (result == null) result = Image.decode(bytes);
                cache.put(digest, result);
            } catch (JSONException | IOException | RuntimeException ignored) {
                // 单张资源不能绘制时不切换到贴图配色，也不保留上一份皮肤的图片。
                fingerprint.append(relative).append("unavailable");
            }
            loaded.put(relative, result);
            return result;
        };
        JSONObject border = manifest.optJSONObject("borderImage");
        Image panel = null;
        if (border != null) {
            String source = border.optString("source");
            if (!dark && !border.optString("lightSource").isEmpty()) source = border.optString("lightSource");
            panel = image.apply(source);
        }
        ArrayList<Widget> widgets = new ArrayList<>();
        JSONArray decorations = manifest.optJSONArray("decorations");
        if (decorations != null) {
            for (int i = 0; i < Math.min(32, decorations.length()); i++) {
                JSONObject item = decorations.getJSONObject(i);
                Image bitmap = image.apply(item.optString("image"));
                if (bitmap != null) widgets.add(new Widget(bitmap, item.optString("anchor", "right"),
                    item.optDouble("widthDip"), item.optDouble("heightDip"), item.optDouble("offsetXDip"),
                    item.optDouble("offsetYDip"), item.optInt("zIndex", 2)));
            }
        }
        widgets.sort(Comparator.comparingInt(Widget::z));
        return new CandidateCanvasAssets(manifest, panel, widgets,
            digest(fingerprint.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)));
    }

    private static JSONObject value(String response) throws JSONException {
        JSONObject envelope = new JSONObject(response);
        return JsonPolicy.strictTrue(envelope.opt("ok")) ? envelope.optJSONObject("value") : null;
    }

    private static String digest(byte[] bytes) {
        try {
            byte[] hash = MessageDigest.getInstance("SHA-256").digest(bytes);
            StringBuilder result = new StringBuilder(64);
            for (byte value : hash) result.append(String.format(java.util.Locale.ROOT, "%02x", value & 255));
            return result.toString();
        } catch (NoSuchAlgorithmException error) { throw new IllegalStateException(error); }
    }

    private static double[] insets(JSONObject value) {
        return value == null ? new double[4] : new double[] {value.optDouble("top"),
            value.optDouble("right"), value.optDouble("bottom"), value.optDouble("left")};
    }

    private static double[] sides(JSONArray values, double[] fallback) {
        if (values == null || (values.length() != 1 && values.length() != 2 && values.length() != 4))
            return fallback.clone();
        double a = values.optDouble(0), b = values.optDouble(values.length() == 1 ? 0 : 1);
        return new double[] {a, b, values.optDouble(values.length() == 4 ? 2 : 0),
            values.optDouble(values.length() == 4 ? 3 : values.length() == 2 ? 1 : 0)};
    }

    record Widget(Image image, String anchor, double width, double height, double x, double y, int z) {}

    /** 独立切片隔绝双线性采样的邻色；静态图和 APNG 在偏好线程裁好，原生动画复用九张缓冲。 */
    static final class Tiles {
        private final Image image;
        private final Rect[] source = new Rect[9];
        private final Map<Bitmap, Bitmap[]> decoded = new java.util.IdentityHashMap<>();
        private Bitmap[] moving;
        private long version = -1;

        Tiles(Image image, double[] slices) {
            this.image = image;
            int[] x = cuts(image.first.getWidth(), slices[3], slices[1]);
            int[] y = cuts(image.first.getHeight(), slices[0], slices[2]);
            for (int row = 0; row < 3; row++) for (int column = 0; column < 3; column++)
                source[row * 3 + column] = new Rect(x[column], y[row], x[column + 1], y[row + 1]);
            if (image.apng != null) for (int i = 0; i < image.apng.size(); i++) prepare(image.apng.frame(i));
            else prepare(image.first);
        }

        private void prepare(Bitmap frame) {
            Bitmap[] tiles = new Bitmap[9];
            for (int i = 0; i < source.length; i++) {
                Rect rect = source[i];
                if (!rect.isEmpty()) tiles[i] = Bitmap.createBitmap(frame, rect.left, rect.top, rect.width(), rect.height());
            }
            decoded.put(frame, tiles);
        }

        Bitmap[] at(Bitmap frame) {
            Bitmap[] tiles = decoded.get(frame);
            if (tiles != null) return tiles;
            if (moving == null) {
                moving = new Bitmap[9];
                for (int i = 0; i < source.length; i++) if (!source[i].isEmpty())
                    moving[i] = Bitmap.createBitmap(source[i].width(), source[i].height(), Bitmap.Config.ARGB_8888);
            }
            if (version != image.version) {
                version = image.version;
                for (int i = 0; i < source.length; i++) if (moving[i] != null) {
                    moving[i].eraseColor(0);
                    new Canvas(moving[i]).drawBitmap(frame, source[i],
                        new Rect(0, 0, source[i].width(), source[i].height()), null);
                }
            }
            return moving;
        }

        private static int[] cuts(int size, double start, double end) {
            double scale = start + end > size ? size / (start + end) : 1;
            return new int[] {0, (int) Math.round(start * scale), (int) Math.round(size - end * scale), size};
        }
    }

    static final class Image implements Drawable.Callback {
        final Bitmap first;
        final CandidateApngFrames apng;
        final AnimatedImageDrawable animated;
        private final WeakHashMap<View, Boolean> hosts = new WeakHashMap<>();
        private final Handler main = new Handler(Looper.getMainLooper());
        private boolean started;
        private Bitmap current;
        private boolean dirty = true;
        private long version;
        private final Map<String, Tiles> tiles = new HashMap<>();

        private Image(Bitmap first, CandidateApngFrames apng, AnimatedImageDrawable animated) {
            this.first = first; this.apng = apng; this.animated = animated;
            if (animated != null) animated.setCallback(this);
        }

        static Image decode(byte[] bytes) throws IOException {
            CandidateApngFrames frames = CandidateApngFrames.decode(bytes);
            if (frames != null) return new Image(frames.frameAt(0, false), frames, null);
            Drawable decoded = ImageDecoder.decodeDrawable(ImageDecoder.createSource(ByteBuffer.wrap(bytes)),
                (decoder, info, source) -> {
                    int w = info.getSize().getWidth(), h = info.getSize().getHeight();
                    if (w < 1 || h < 1 || w > 2048 || h > 2048 || (long) w * h > 8_000_000)
                        throw new IllegalArgumentException("candidate image dimensions");
                    decoder.setAllocator(ImageDecoder.ALLOCATOR_SOFTWARE);
                    decoder.setOnPartialImageListener(error -> false);
                });
            Bitmap first;
            if (decoded instanceof BitmapDrawable bitmap) first = bitmap.getBitmap();
            else {
                first = Bitmap.createBitmap(decoded.getIntrinsicWidth(), decoded.getIntrinsicHeight(), Bitmap.Config.ARGB_8888);
                decoded.setBounds(0, 0, first.getWidth(), first.getHeight());
                decoded.draw(new Canvas(first));
            }
            return new Image(first, null, decoded instanceof AnimatedImageDrawable animation ? animation : null);
        }

        synchronized Tiles tiles(double[] slices) {
            String key = java.util.Arrays.toString(slices);
            // 清单可原地改切线，旧 View 仍持有自己的切片；缓存只留最近两种几何。
            if (!tiles.containsKey(key) && tiles.size() >= 2) tiles.clear();
            return tiles.computeIfAbsent(key, ignored -> new Tiles(this, slices));
        }

        void visible(View host, boolean visible) {
            if (visible) hosts.put(host, true); else hosts.remove(host);
            boolean play = ValueAnimator.areAnimatorsEnabled() && hosts.keySet().stream().anyMatch(View::isShown);
            if (animated != null) {
                if (play && !started) { started = true; animated.start(); }
                else if (!play && started) { animated.stop(); started = false; }
            }
        }

        Bitmap bitmapAt(long elapsed, boolean motion) {
            if (apng != null) return apng.frameAt(elapsed, motion);
            if (animated == null || !motion) return first;
            if (current == null) current = Bitmap.createBitmap(first.getWidth(), first.getHeight(), Bitmap.Config.ARGB_8888);
            if (dirty) {
                dirty = false;
                current.eraseColor(0);
                animated.draw(new Canvas(current));
            }
            return current;
        }

        @Override public void invalidateDrawable(Drawable who) {
            dirty = true;
            version++;
            for (View host : hosts.keySet()) if (host.isShown()) host.invalidate();
        }
        @Override public void scheduleDrawable(Drawable who, Runnable what, long when) {
            main.postAtTime(what, who, when);
        }
        @Override public void unscheduleDrawable(Drawable who, Runnable what) { main.removeCallbacks(what, who); }
    }
}
