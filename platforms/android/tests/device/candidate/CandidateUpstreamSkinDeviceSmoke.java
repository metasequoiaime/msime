package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** 固定上游的完整资源在真实候选栏与展开面板验收；只使用独立编辑器的合成输入。 */
public final class CandidateUpstreamSkinDeviceSmoke extends DeviceSmoke {
    private String captureRun;

    @Override public void onCreate(Bundle arguments) {
        captureRun = arguments == null ? "" : arguments.getString("capture_run", "");
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return "fixed upstream skins: two packages, light/dark, live strip/expanded panel, eight captures and commit";
    }

    @Override protected void runChecks() throws Exception {
        check(captureRun.matches("upstream-[0-9-]+"), "explicit capture run is required");
        String[] ids = {"green_leaf", "sakura-night"};
        stage = "fixed upstream assets must be explicitly staged";
        for (String id : ids) {
            try (var manifest = getContext().getAssets().open("upstream-candidate-skins/" + id + "/skin.toml")) {
                check(manifest.read() != -1, "upstream manifest missing");
            }
        }
        Path root = getTargetContext().getFilesDir().getCanonicalFile().toPath();
        JSONObject options = json(root.resolve("runtime-options.json"));
        Path state = new File(options.getString("preferences_directory")).getCanonicalFile().toPath();
        check(state.startsWith(root), "state outside application sandbox");
        Path preferences = state.resolve("preferences.json");
        byte[] original = Files.exists(preferences) ? Files.readAllBytes(preferences) : null;
        List<Path> installed = new ArrayList<>();
        Path output = getTargetContext().getExternalFilesDir(null).toPath().resolve(captureRun);
        Files.createDirectory(output);
        try {
            Path skins = Files.createDirectories(state.resolve("skins"));
            for (String id : ids) {
                Path folder = Files.createDirectory(skins.resolve(id));
                installed.add(folder);
                copyAsset("upstream-candidate-skins/" + id, folder);
            }
            long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8))
                .getLong("revision");
            JSONObject chosen = new JSONObject(nativeCall("defaultPreferences", null)).getJSONObject("value")
                .put("global_theme", "custom").put("cloud_candidates", false).put("learning", false)
                .put("candidate_english_gloss", false).put("scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                    .put("selected", "quanpin"));
            for (String id : ids) for (boolean dark : new boolean[] {false, true}) {
                String mode = dark ? "dark" : "light";
                stage = id + " " + mode + " full resource decoding";
                chosen.put("theme", mode).put("candidate_theme", mode).put("screen_keyboard_theme", mode)
                    .put("custom_theme", new JSONObject().put("base", "system")
                        .put("candidate_skin", id).put("candidate_skin_dark", id));
                save(preferences, chosen, ++revision);
                checkCanvas(state, id, dark, id.equals("green_leaf") ? 2 : 3);
                shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                tap(key("n"));
                tap(key("i"));
                awaitStableBounds(describedPrefix("候选 2："));
                SystemClock.sleep(400);
                stage = id + " " + mode + " live candidate strip raster";
                Rect keyboardBounds = imeBounds();
                Rect stripBounds = new Rect();
                awaitStableBounds(described("展开候选")).getBoundsInScreen(stripBounds);
                capture(output.resolve(id + "-" + mode + "-strip.png"), false);
                tap(described("展开候选"));
                awaitStableBounds(described("候选面板"));
                SystemClock.sleep(1500);
                stage = id + " " + mode + " expanded candidate raster";
                capture(output.resolve(id + "-" + mode + "-expanded.png"), true);
                check(keyboardBounds.equals(imeBounds()), "expanded canvas must preserve keyboard measurement");
                for (AccessibilityWindowInfo window : automation.getWindows()) {
                    check(find(window.getRoot(), key("q")) == null,
                        "expanded translucent canvas must hide the underlying letter keys");
                }
                Rect retainedStrip = new Rect();
                awaitStableBounds(described("展开候选")).getBoundsInScreen(retainedStrip);
                check(stripBounds.equals(retainedStrip), "candidate strip must remain above the expanded canvas");
                tap(described(dark ? "展开候选" : "收起候选面板"));
                awaitStableBounds(key("q"));
                awaitStableBounds(described("展开候选"));
                check(keyboardBounds.equals(imeBounds()), "closing canvas must restore the keyboard dimensions");
                tap(describedPrefix("候选 2："));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && !node.getText().toString().endsWith("ni")));
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences);
            else publish(preferences.toFile(), original);
            for (Path folder : installed) try (var paths = Files.walk(folder)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toArray(Path[]::new)) Files.delete(path);
            }
        }
    }

    private void checkCanvas(Path state, String id, boolean dark, int widgets) throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Object resolver = loader.loadClass("app.msime.android.CandidateSkinResolver").getConstructor().newInstance();
        JSONObject loaded = new JSONObject((String) resolver.getClass().getMethod("read", String.class)
            .invoke(resolver, state.toString()));
        check(loaded.optBoolean("ok"), "upstream preferences rejected");
        Class<?> skin = loader.loadClass("app.msime.android.KeyboardSkin");
        Class<?> palette = loader.loadClass("app.msime.android.CandidateAppearance$Palette");
        Object fallback = loader.loadClass("app.msime.android.CandidateAppearance").getMethod("fromSkin", skin)
            .invoke(null, skin.getMethod("system", boolean.class).invoke(null, dark));
        Object selected = resolver.getClass().getMethod("paletteFor", String.class, JSONObject.class, boolean.class, palette)
            .invoke(resolver, state.toString(), loaded.getJSONObject("value").getJSONObject("preferences"), dark, fallback);
        check(id.equals(selected.getClass().getMethod("id").invoke(selected)), "upstream package did not resolve");
        Object canvas = selected.getClass().getMethod("canvas").invoke(selected);
        check(canvas != null && (Boolean) canvas.getClass().getMethod("panelDrawn").invoke(canvas),
            "upstream panel fell back to a plain surface");
        Field field = canvas.getClass().getDeclaredField("widgets");
        field.setAccessible(true);
        check(((List<?>) field.get(canvas)).size() == widgets, "an upstream decoration failed to decode");
    }

    private void capture(Path path, boolean expanded) throws Exception {
        Rect area = new Rect();
        if (expanded) {
            // 截取完整 IME，避免长列表的可访问边界裁掉外留白、顶部挂件或返回键。
            area.set(imeBounds());
        } else {
            awaitStableBounds(describedPrefix("候选 2：")).getBoundsInScreen(area);
            area.top -= dp(96);
            area.bottom += dp(32);
            area.left = 0;
            area.right = getTargetContext().getResources().getDisplayMetrics().widthPixels;
        }
        Rect ime = imeBounds();
        check(!ime.isEmpty() && area.intersect(ime), "IME raster bounds unavailable");
        Bitmap screen = automation.takeScreenshot();
        check(screen != null, "IME screenshot unavailable");
        try {
            check(area.intersect(0, 0, screen.getWidth(), screen.getHeight()), "IME raster outside screen");
            Bitmap cropped = Bitmap.createBitmap(screen, area.left, area.top, area.width(), area.height());
            try (var output = Files.newOutputStream(path)) {
                check(cropped.compress(Bitmap.CompressFormat.PNG, 100, output), "IME raster encoding failed");
            } finally { if (cropped != screen) cropped.recycle(); }
        } finally { screen.recycle(); }
    }

    private Rect imeBounds() {
        Rect bounds = new Rect();
        for (AccessibilityWindowInfo window : automation.getWindows())
            if (window.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD) window.getBoundsInScreen(bounds);
        check(!bounds.isEmpty(), "IME window unavailable");
        return bounds;
    }

    private int dp(int value) {
        return Math.round(value * getTargetContext().getResources().getDisplayMetrics().density);
    }

    private void copyAsset(String source, Path destination) throws Exception {
        String[] children = getContext().getAssets().list(source);
        if (children != null && children.length > 0) {
            Files.createDirectories(destination);
            for (String child : children) {
                check(!child.contains("/") && !child.equals(".."), "invalid staged asset key");
                copyAsset(source + "/" + child, destination.resolve(child));
            }
        } else try (var input = getContext().getAssets().open(source)) { Files.copy(input, destination); }
    }

    private JSONObject json(Path file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file), StandardCharsets.UTF_8));
    }

    private void save(Path file, JSONObject preferences, long revision) throws Exception {
        publish(file.toFile(), new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8));
    }

    private String nativeCall(String name, String request) throws Exception {
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        return (String) (request == null ? type.getMethod(name).invoke(null)
            : type.getMethod(name, String.class).invoke(null, request));
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
