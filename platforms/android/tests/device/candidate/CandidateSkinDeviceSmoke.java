package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import java.io.File;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import org.json.JSONObject;
import org.json.JSONArray;

/** 用合成包检验真实 JNI 的目录限制、明暗槽位及候选栏绘制；不访问用户输入。 */
public final class CandidateSkinDeviceSmoke extends DeviceSmoke {
    private static final String LIGHT = "candidate-smoke-light";
    private static final String DARK = "candidate-smoke-dark";

    @Override protected String successDescription() {
        return "candidate packages: private storage, slots, layout, invalidation and rendered chip colours";
    }

    @Override protected void runChecks() throws Exception {
        stage = "package resolver in application private storage";
        Path scratch = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "candidate-skin-");
        try { checkResolver(scratch); }
        finally { removeFixture(scratch); }

        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        Path state = new File(options.getString("preferences_directory")).getCanonicalFile().toPath();
        check(state.startsWith(root.getCanonicalFile().toPath()), "state outside application sandbox");
        Path skinRoot = Files.createDirectories(state.resolve("skins"));
        // 只清理本次独占创建的两个合成目录；碰到同名目录时直接失败。
        Path light = Files.createDirectory(skinRoot.resolve(LIGHT));
        Path dark = null;
        Path preferences = state.resolve("preferences.json");
        byte[] original = Files.exists(preferences) ? Files.readAllBytes(preferences) : null;
        try {
            dark = Files.createDirectory(skinRoot.resolve(DARK));
            writeSkin(light, LIGHT, "light", "horizontal", "#135724");
            writeSkin(dark, DARK, "night", "horizontal", "#572413");
            long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8))
                .getLong("revision");
            JSONObject chosen = new JSONObject(options.getJSONObject("preferences").toString())
                .put("global_theme", "custom").put("theme", "light").put("candidate_theme", "light")
                .put("screen_keyboard_theme", "light").put("cloud_candidates", false)
                .put("learning", false).put("candidate_english_gloss", false)
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                    .put("selected", "quanpin"))
                .put("custom_theme", custom());
            save(preferences, chosen, ++revision);
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            tap(key("n"));
            tap(key("i"));
            stage = "light package reaches the candidate button";
            awaitCandidateColour(0xff135724);
            int keyboardColour = dominantColour(bounds(key("n")));
            tap(key("空格"));
            stage = "candidate dark override preserves the keyboard palette";
            chosen.put("candidate_theme", "dark");
            save(preferences, chosen, ++revision);
            SystemClock.sleep(1500);
            tap(key("n"));
            tap(key("i"));
            awaitCandidateColour(0xff572413);
            check(dominantColour(bounds(key("n"))) == keyboardColour, "candidate package recoloured keyboard");
            stage = "manifest update repaints without a preference revision";
            writeSkin(dark, DARK, "night", "horizontal", "#461357");
            awaitCandidateColour(0xff461357);
            stage = "canvas texture reaches the live candidate strip";
            writeCanvas(dark, DARK);
            awaitCandidateColour(0xff314159);
            check(dominantColour(bounds(describedPrefix("候选 2："))) == 0xff28465a,
                "unselected candidate covered the panel texture");
            check(dominantColour(bounds(key("n"))) == keyboardColour, "canvas recoloured keyboard keys");
            stage = "expanded candidates retain a fixed canvas";
            tap(described("展开候选"));
            check(dominantColour(bounds(described("候选面板"))) == 0xff28465a,
                "expanded candidate surface did not draw the canvas");
            tap(described("收起候选面板"));
            stage = "canvas keeps the original candidate click path";
            tap(describedPrefix("候选 2："));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && !node.getText().toString().endsWith("ni")));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences);
            else publish(preferences.toFile(), original);
            removeFixture(light);
            if (dark != null) removeFixture(dark);
        }
    }

    private void checkResolver(Path state) throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> nativeType = loader.loadClass("app.msime.android.NativeClient");
        Class<?> resolverType = loader.loadClass("app.msime.android.CandidateSkinResolver");
        Class<?> skinType = loader.loadClass("app.msime.android.KeyboardSkin");
        Class<?> appearanceType = loader.loadClass("app.msime.android.CandidateAppearance");
        Class<?> paletteType = loader.loadClass("app.msime.android.CandidateAppearance$Palette");
        Object resolver = resolverType.getConstructor().newInstance();
        Object fallback = appearanceType.getMethod("fromSkin", skinType)
            .invoke(null, skinType.getMethod("system", boolean.class).invoke(null, false));
        Method read = resolverType.getMethod("read", String.class);
        Method palette = resolverType.getMethod("paletteFor", String.class, JSONObject.class,
            boolean.class, paletteType);
        JSONObject defaults = new JSONObject((String) nativeType.getMethod("defaultPreferences").invoke(null))
            .getJSONObject("value").put("global_theme", "custom").put("custom_theme", custom());
        Path light = Files.createDirectories(state.resolve("skins/" + LIGHT));
        Path dark = Files.createDirectories(state.resolve("skins/" + DARK));
        writeSkin(light, LIGHT, "light", "horizontal", "#135724");
        writeSkin(dark, DARK, "night", "horizontal", "#572413");
        save(state.resolve("preferences.json"), defaults, 1);
        JSONObject loaded = new JSONObject((String) read.invoke(resolver, state.toString()));
        check(loaded.optBoolean("ok"), "fixture preferences were not accepted");
        // 与产品相同，主线程消费共享层补齐默认字段后的快照，而非写盘前的稀疏对象。
        defaults = loaded.getJSONObject("value").getJSONObject("preferences");
        Object selected = palette.invoke(resolver, state.toString(), defaults, false, fallback);
        check(LIGHT.equals(property(selected, "id")), "light slot was not loaded");
        check(property(selected, "chip").equals(0xff135724), "light palette was not resolved");
        check(property(palette.invoke(resolver, state.toString(), defaults, true, fallback), "id").equals(DARK),
            "dark slot was not loaded");
        check(palette.invoke(resolver, state.resolve("other").toString(), defaults, false, fallback) == fallback,
            "another state directory reused the old palette");
        JSONObject switched = new JSONObject(defaults.toString()).put("global_theme", "system");
        check(palette.invoke(resolver, state.toString(), switched, false, fallback) == fallback,
            "new theme reused the old package");
        writeSkin(light, LIGHT, "light", "vertical", "#135724");
        read.invoke(resolver, state.toString());
        check(palette.invoke(resolver, state.toString(), defaults, false, fallback) == fallback,
            "vertical-only package entered the touch strip");
        writeSkin(light, LIGHT, "light", "horizontal", "#246813");
        read.invoke(resolver, state.toString());
        check(property(palette.invoke(resolver, state.toString(), defaults, false, fallback), "chip")
            .equals(0xff246813), "unchanged preferences hid a manifest replacement");
        stage = "asset deletion invalidates a package without a manifest change";
        Path image = light.resolve("fixture.png");
        Bitmap bitmap = Bitmap.createBitmap(4, 4, Bitmap.Config.ARGB_8888);
        try (var output = new java.io.ByteArrayOutputStream()) {
            bitmap.eraseColor(0xff123456);
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output), "synthetic PNG encoding failed");
            Files.write(image, output.toByteArray());
        } finally { bitmap.recycle(); }
        Path manifestPath = light.resolve("skin.toml");
        String decorated = new String(Files.readAllBytes(manifestPath), StandardCharsets.UTF_8)
            .replace("top_inset_dip = 0, width_dip = 0",
                "top_inset_dip = 1, width_dip = 1, image = 'fixture.png'");
        Files.write(manifestPath, decorated.getBytes(StandardCharsets.UTF_8));
        read.invoke(resolver, state.toString());
        check(property(palette.invoke(resolver, state.toString(), defaults, false, fallback), "externalSkin")
            .equals(true), "valid decorated package was rejected");
        Files.delete(image);
        read.invoke(resolver, state.toString());
        check(palette.invoke(resolver, state.toString(), defaults, false, fallback) == fallback,
            "missing image retained the package palette");
        stage = "malformed and missing manifests restore the host palette";
        Files.write(light.resolve("skin.toml"), "schema_version = 'invalid'\n".getBytes(StandardCharsets.UTF_8));
        read.invoke(resolver, state.toString());
        check(palette.invoke(resolver, state.toString(), defaults, false, fallback) == fallback,
            "invalid package retained its colours");
        Files.delete(light.resolve("skin.toml"));
        read.invoke(resolver, state.toString());
        check(palette.invoke(resolver, state.toString(), defaults, false, fallback) == fallback,
            "missing package retained its colours");
    }

    private static JSONObject custom() throws Exception {
        return new JSONObject().put("base", "system").put("candidate_skin", LIGHT)
            .put("candidate_skin_dark", DARK);
    }

    private void save(Path path, JSONObject preferences, long revision) throws Exception {
        publish(path.toFile(), new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void writeSkin(Path path, String id, String base, String layout, String selected)
            throws Exception {
        String mode = "night".equals(base) ? "dark" : "light";
        String manifest = "schema_version = 1\nid = '" + id
            + "'\nname = 'Synthetic candidate fixture'\nversion = '1.0'\nbase = '" + base
            + "'\n[supports]\nlayouts = ['" + layout + "']\nthemes = ['" + mode
            + "']\n[candidate_window]\nmin_width_dip = 10\n"
            + "decoration = { top_inset_dip = 0, width_dip = 0 }\n[candidate." + mode
            + "]\nselected = '" + selected + "'\ntext = '#123456'\nsurface = '#E9E1D2'\n";
        Files.write(path.resolve("skin.toml"), manifest.getBytes(StandardCharsets.UTF_8));
    }

    private static void writeCanvas(Path path, String id) throws Exception {
        Bitmap bitmap = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888);
        try (var output = Files.newOutputStream(path.resolve("panel.png"))) {
            bitmap.eraseColor(0xff28465a);
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output), "synthetic panel encoding");
        } finally { bitmap.recycle(); }
        String manifest = "schema_version=2\nid='" + id
            + "'\nname='Synthetic canvas integration'\nversion='1.0'\nbase='canvas'\n"
            + "[supports]\nlayouts=['horizontal']\nthemes=['dark','light']\n"
            + "[candidate_window]\ncontent_insets_dip={top=4,right=6,bottom=4,left=6}\n"
            + "outer_insets_dip={top=2,right=2,bottom=2,left=2}\nline_gap_dip=4\n"
            + "[candidate_window.border_image]\nsource='panel.png'\nslice_dip=[2]\nwidth_dip=[2]\n"
            + "[candidate.dark]\ntext='#FFFFFF'\nselected='#314159'\n"
            + "[candidate.light]\ntext='#FFFFFF'\nselected='#314159'\n";
        Files.write(path.resolve("skin.toml"), manifest.getBytes(StandardCharsets.UTF_8));
    }

    private static Object property(Object object, String name) throws Exception {
        return object.getClass().getMethod(name).invoke(object);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void removeFixture(Path path) throws Exception {
        try (var paths = Files.walk(path)) {
            for (Path entry : paths.sorted(Comparator.reverseOrder()).toArray(Path[]::new))
                Files.delete(entry);
        }
    }

    private Rect bounds(java.util.function.Predicate<android.view.accessibility.AccessibilityNodeInfo> match) {
        Rect result = new Rect();
        awaitStableBounds(match).getBoundsInScreen(result);
        return result;
    }

    /** 截图只在内存统计本测试候选按钮的颜色，不保存编辑器或其他窗口。 */
    private int dominantColour(Rect area) {
        Bitmap screenshot = automation.takeScreenshot();
        if (screenshot == null) throw new AssertionError("Screenshot unavailable");
        try {
            java.util.Map<Integer, Integer> counts = new java.util.HashMap<>();
            int best = 0, count = 0;
            for (int y = area.top + 2; y < Math.min(area.bottom - 2, screenshot.getHeight()); y += 3) {
                for (int x = area.left + 2; x < Math.min(area.right - 2, screenshot.getWidth()); x += 3) {
                    int colour = screenshot.getPixel(x, y);
                    int total = counts.getOrDefault(colour, 0) + 1;
                    counts.put(colour, total);
                    if (total > count) { best = colour; count = total; }
                }
            }
            check(count > 0, "control has no visible pixels");
            return best;
        } finally { screenshot.recycle(); }
    }

    private void awaitCandidateColour(int colour) {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            if (dominantColour(bounds(describedPrefix("候选 1："))) == colour) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("candidate chip did not use the installed package palette");
    }
}
