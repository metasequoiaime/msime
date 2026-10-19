package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.drawable.AnimatedImageDrawable;
import android.graphics.drawable.Drawable;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.lang.reflect.InvocationTargetException;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.Comparator;
import java.util.zip.CRC32;
import java.util.zip.DeflaterOutputStream;
import org.json.JSONObject;

/** 在设备上调用产品解码器、JNI 和画布 View；所有资源和像素均为合成值。 */
public final class CandidateCanvasDeviceSmoke extends DeviceSmoke {
    private ClassLoader loader;
    private Class<?> assetsType;
    private float density;

    @Override protected String successDescription() {
        return "candidate canvas: native rasters, nine-slice, widgets, insets, APNG/GIF/WebP and invalidation";
    }

    @Override protected void runChecks() throws Exception {
        loader = getTargetContext().getClassLoader();
        assetsType = loader.loadClass("app.msime.android.CandidateCanvasAssets");
        density = getTargetContext().getResources().getDisplayMetrics().density;
        stage = "APNG offsets, blend, disposal, delays and finite loops";
        apng();
        stage = "GIF and WebP native frames";
        animated("R0lGODlhDAAMAIEAAP8AAAAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEAAAAh+QQAEgAAACwAAAAADAAMAAAIFgABCBxIsKDBgwgTKlzIsKHDhxARBgQAIfkEARIAAQAsAAAAAAwADACBAAD/AAAAAAAAAAAACBYAAQgcSLCgwYMIEypcyLChw4cQEQYEADs=");
        animated("UklGRoQAAABXRUJQVlA4WAoAAAACAAAACwAACwAAQU5JTQYAAAAAAAAAAABBTk1GKAAAAAAAAAAAAAsAAAsAALQAAAJWUDhMDwAAAC8LwAIABxD9j/4HIqL/AQBBTk1GKAAAAAAAAAAAAAsAAAsAALQAAABWUDhMDwAAAC8LwAIABxDR//4HIqL/AQA=");
        Path state = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "canvas-device-");
        try { raster(state); }
        finally {
            try (var paths = Files.walk(state)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toArray(Path[]::new)) Files.delete(path);
            }
        }
    }

    private void raster(Path state) throws Exception {
        stage = "canvas in private application storage";
        Path skin = Files.createDirectories(state.resolve("skins/canvas-device"));
        int[] colours = {0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffff00, 0xffffffff,
            0xff00ffff, 0xffff00ff, 0xffff8800, 0xff777777};
        png(skin.resolve("atlas.png"), 3, 3, colours);
        png(skin.resolve("light.png"), 3, 3, new int[] {0xff884422, 0xff884422, 0xff884422,
            0xff884422, 0xff884422, 0xff884422, 0xff884422, 0xff884422, 0xff884422});
        png(skin.resolve("widget.png"), 1, 1, new int[] {0xff1234ab});
        String manifest = "schema_version=2\nid='canvas-device'\nname='Synthetic canvas'\nversion='1.0'\nbase='canvas'\n"
            + "[supports]\nlayouts=['horizontal','vertical']\nthemes=['light','dark']\n"
            + "[candidate_window]\ncontent_insets_dip={top=2,right=3,bottom=4,left=5}\n"
            + "outer_insets_dip={top=4,right=6,bottom=8,left=10}\n"
            + "[candidate_window.border_image]\nsource='atlas.png'\nlight_source='light.png'\nslice_dip=[1]\nwidth_dip=[2]\nfill=true\n"
            + "[[candidate_window.decorations]]\nimage='widget.png'\nanchor='center'\nwidth_dip=12\nheight_dip=12\nz_index=0\n"
            + "[[candidate_window.decorations]]\nimage='widget.png'\nanchor='top-left'\nwidth_dip=6\nheight_dip=6\noffset_x_dip=5\noffset_y_dip=3\nz_index=2\n"
            + "[candidate.dark]\ntext='#F7E6D5'\nselected='#163B57'\n[candidate.light]\ntext='#112233'\nselected='#572413'\n";
        Files.write(skin.resolve("skin.toml"), manifest.getBytes(StandardCharsets.UTF_8));
        JSONObject packageEnvelope = new JSONObject(nativeCall("skinPackage", new JSONObject()
            .put("directory", state.resolve("skins").toString()).put("id", "canvas-device").toString()));
        check(packageEnvelope.optBoolean("ok"), "synthetic canvas package rejected: " + packageEnvelope.optString("error"));
        JSONObject preferences = new JSONObject(nativeCall("defaultPreferences", null)).getJSONObject("value")
            .put("global_theme", "custom").put("custom_theme", new JSONObject().put("base", "system")
                .put("candidate_skin", "canvas-device").put("candidate_skin_dark", "canvas-device"));
        publish(state.resolve("preferences.json").toFile(), new JSONObject().put("format_version", 1)
            .put("revision", 1).put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8));
        Object resolver = loader.loadClass("app.msime.android.CandidateSkinResolver").getConstructor().newInstance();
        JSONObject loaded = new JSONObject((String) call(resolver, "read", new Class<?>[] {String.class}, state.toString()));
        check(loaded.optBoolean("ok"), "canvas preferences rejected");
        preferences = loaded.getJSONObject("value").getJSONObject("preferences");
        Object dark = resolve(resolver, state, preferences, true);
        Object light = resolve(resolver, state, preferences, false);
        Object canvas = property(dark, "canvas");
        check(canvas != null && property(canvas, "panelDrawn").equals(true),
            "panel did not decode for " + property(dark, "id"));
        check(property(dark, "text").equals(0xfff7e6d5), "successful texture kept fallback text");
        for (String view : new String[] {"CandidateCanvasColumn", "CandidateCanvasFrame"}) {
            stage = view + " panel and widget layers";
            ViewGroup host = (ViewGroup) loader.loadClass("app.msime.android." + view)
                .getConstructor(android.content.Context.class).newInstance(getTargetContext());
            View child = new View(getTargetContext());
            child.setBackgroundColor(0xffffff00);
            runOnMainSync(() -> host.addView(child, new ViewGroup.LayoutParams(px(10), px(10))));
            Bitmap image = render(host, canvas);
            try {
                pixel(image, 11, 5, 0xffff0000, "top-left slice");
                pixel(image, 80, 5, 0xff00ff00, "top edge");
                pixel(image, 153, 91, 0xff777777, "bottom-right slice");
                pixel(image, 80, 48, 0xffffffff, "negative widget must be below opaque panel");
                pixel(image, 18, 10, 0xff1234ab, "positive widget must cover child content");
                pixel(image, 24, 10, 0xffffff00, "original child still draws beside widget");
                pixel(image, 2, 2, 0x00000000, "outer inset must stay clear");
                check(host.getPaddingLeft() == px(15) && host.getPaddingTop() == px(6)
                    && host.getPaddingRight() == px(9) && host.getPaddingBottom() == px(12), "four-sided padding");
            } finally { image.recycle(); }
            image = render(host, property(light, "canvas"));
            try { pixel(image, 80, 48, 0xff884422, "same-package light image"); }
            finally { image.recycle(); }
            check(host.getChildAt(0) == child, "theme switch replaced candidate child");
        }
        stage = "asset updates without preference revision";
        String oldKey = (String) property(canvas, "key");
        png(skin.resolve("atlas.png"), 3, 3, new int[] {0xff111111, 0xff111111, 0xff111111,
            0xff111111, 0xff111111, 0xff111111, 0xff111111, 0xff111111, 0xff111111});
        call(resolver, "read", new Class<?>[] {String.class}, state.toString());
        check(!oldKey.equals(property(property(resolve(resolver, state, preferences, true), "canvas"), "key")),
            "updated bytes retained old canvas");
        stage = "missing texture restores readable palette";
        Files.delete(skin.resolve("atlas.png"));
        call(resolver, "read", new Class<?>[] {String.class}, state.toString());
        check(property(resolve(resolver, state, preferences, true), "canvas") == null, "missing texture retained old scene");
    }

    private Object resolve(Object resolver, Path state, JSONObject preferences, boolean dark) throws Exception {
        Class<?> skin = loader.loadClass("app.msime.android.KeyboardSkin");
        Object fallback = loader.loadClass("app.msime.android.CandidateAppearance").getMethod("fromSkin", skin)
            .invoke(null, skin.getMethod("system", boolean.class).invoke(null, dark));
        return call(resolver, "paletteFor", new Class<?>[] {String.class, JSONObject.class, boolean.class,
            loader.loadClass("app.msime.android.CandidateAppearance$Palette")}, state.toString(), preferences, dark, fallback);
    }

    private Bitmap render(View host, Object canvas) throws Exception {
        Bitmap output = Bitmap.createBitmap(px(160), px(100), Bitmap.Config.ARGB_8888);
        Throwable[] error = new Throwable[1];
        runOnMainSync(() -> {
            try {
                call(host, "apply", new Class<?>[] {assetsType, boolean.class, int.class}, canvas, true, 0xff202020);
                host.measure(View.MeasureSpec.makeMeasureSpec(output.getWidth(), View.MeasureSpec.EXACTLY),
                    View.MeasureSpec.makeMeasureSpec(output.getHeight(), View.MeasureSpec.EXACTLY));
                host.layout(0, 0, output.getWidth(), output.getHeight());
                host.draw(new Canvas(output));
            } catch (Exception failure) { error[0] = failure; }
        });
        if (error[0] != null) throw new AssertionError("product canvas draw failed", error[0]);
        return output;
    }

    private void apng() throws Exception {
        Class<?> type = loader.loadClass("app.msime.android.CandidateApngFrames");
        byte[] bytes = apngBytes();
        Object animation = type.getMethod("decode", byte[].class).invoke(null, (Object) bytes);
        check(property(animation, "size").equals(4), "APNG frame count");
        check(property(animation, "durationMillis").equals(400L), "APNG delays");
        int[][] expected = {{0xffff0000, 0xffff0000}, {0xff00ff00, 0xffff0000},
            {0xffff0000, 0xff0000ff}, {0xffffffff, 0x00000000}};
        for (int i = 0; i < 4; i++) {
            Bitmap frame = (Bitmap) call(animation, "frameAt", new Class<?>[] {long.class, boolean.class}, i * 100L, true);
            check(frame.getPixel(0, 0) == expected[i][0] && frame.getPixel(1, 0) == expected[i][1], "APNG composition " + i);
        }
        Bitmap frozen = (Bitmap) call(animation, "frameAt", new Class<?>[] {long.class, boolean.class}, 2000L, false);
        check(frozen.getPixel(0, 0) == 0xffff0000, "reduced-motion first frame");
        Bitmap finalFrame = (Bitmap) call(animation, "frameAt", new Class<?>[] {long.class, boolean.class}, 2000L, true);
        check(finalFrame.getPixel(1, 0) == 0 && finalFrame.getPixel(0, 0) == 0xffffffff, "finite animation final frame");
        bytes[bytes.length - 1] ^= 1;
        try {
            type.getMethod("decode", byte[].class).invoke(null, (Object) bytes);
            throw new AssertionError("corrupt APNG was accepted");
        } catch (InvocationTargetException failure) {
            check(failure.getCause() instanceof java.io.IOException, "unexpected decoder error");
        }
    }

    private void animated(String encoded) throws Exception {
        Class<?> imageType = loader.loadClass("app.msime.android.CandidateCanvasAssets$Image");
        var decode = imageType.getDeclaredMethod("decode", byte[].class); decode.setAccessible(true);
        Object image = decode.invoke(null, (Object) Base64.getDecoder().decode(encoded));
        var field = imageType.getDeclaredField("animated"); field.setAccessible(true);
        AnimatedImageDrawable drawable = (AnimatedImageDrawable) field.get(image);
        check(drawable != null, "native animated format became a static image");
        int[] colour = new int[1];
        runOnMainSync(() -> { colour[0] = drawPixel(drawable); drawable.start(); });
        boolean changed = false;
        try {
            for (int i = 0; i < 30; i++) {
                SystemClock.sleep(30);
                int[] next = new int[1];
                runOnMainSync(() -> next[0] = drawPixel(drawable));
                if (next[0] != colour[0]) { changed = true; break; }
            }
        } finally { runOnMainSync(drawable::stop); }
        check(changed, "native GIF/WebP frames did not advance");
    }

    private static int drawPixel(Drawable drawable) {
        Bitmap image = Bitmap.createBitmap(12, 12, Bitmap.Config.ARGB_8888);
        try { drawable.setBounds(0, 0, 12, 12); drawable.draw(new Canvas(image)); return image.getPixel(6, 6); }
        finally { image.recycle(); }
    }

    private byte[] apngBytes() throws Exception {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(new byte[] {(byte) 137, 80, 78, 71, 13, 10, 26, 10});
        chunk(out, "IHDR", ByteBuffer.allocate(13).putInt(2).putInt(1).put((byte) 8).put((byte) 6).put(new byte[3]).array());
        chunk(out, "acTL", ByteBuffer.allocate(8).putInt(4).putInt(1).array());
        int sequence = 0;
        int[][] rgba = {{255,0,0,255,255,0,0,255}, {0,255,0,255}, {0,0,255,255}, {255,255,255,255}};
        for (int i = 0; i < 4; i++) {
            chunk(out, "fcTL", ByteBuffer.allocate(26).putInt(sequence++).putInt(i == 0 ? 2 : 1).putInt(1)
                .putInt(i == 2 ? 1 : 0).putInt(0).putShort((short) 1).putShort((short) 10)
                .put((byte) (i == 1 ? 2 : i == 2 ? 1 : 0)).put((byte) 0).array());
            ByteArrayOutputStream zipped = new ByteArrayOutputStream();
            try (DeflaterOutputStream deflate = new DeflaterOutputStream(zipped)) {
                deflate.write(0);
                for (int value : rgba[i]) deflate.write(value);
            }
            if (i == 0) chunk(out, "IDAT", zipped.toByteArray());
            else chunk(out, "fdAT", ByteBuffer.allocate(zipped.size() + 4).putInt(sequence++).put(zipped.toByteArray()).array());
        }
        chunk(out, "IEND", new byte[0]);
        return out.toByteArray();
    }

    private static void chunk(ByteArrayOutputStream out, String type, byte[] bytes) throws Exception {
        byte[] name = type.getBytes(StandardCharsets.US_ASCII);
        CRC32 crc = new CRC32(); crc.update(name); crc.update(bytes);
        DataOutputStream data = new DataOutputStream(out);
        data.writeInt(bytes.length); data.write(name); data.write(bytes); data.writeInt((int) crc.getValue());
    }

    private static void png(Path file, int width, int height, int[] pixels) throws Exception {
        Bitmap bitmap = Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888);
        try (var out = Files.newOutputStream(file)) {
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, out), "synthetic PNG encoding");
        } finally { bitmap.recycle(); }
    }

    private int px(float dp) { return Math.round(dp * density); }
    private void pixel(Bitmap image, float x, float y, int expected, String label) {
        int actual = image.getPixel(px(x), px(y));
        check(actual == expected, label + ": expected " + Integer.toHexString(expected) + ", got " + Integer.toHexString(actual));
    }
    private String nativeCall(String name, String request) throws Exception {
        Class<?> type = loader.loadClass("app.msime.android.NativeClient");
        return (String) (request == null ? type.getMethod(name).invoke(null)
            : type.getMethod(name, String.class).invoke(null, request));
    }
    private static Object property(Object object, String name) throws Exception { return call(object, name, new Class<?>[0]); }
    private static Object call(Object object, String name, Class<?>[] types, Object... values) throws Exception {
        return object.getClass().getMethod(name, types).invoke(object, values);
    }
    private static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }
}
