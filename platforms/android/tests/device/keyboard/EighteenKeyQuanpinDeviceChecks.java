package app.msime.android.test;

import android.graphics.Rect;
import android.content.res.Configuration;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONObject;

/** 复用既有 instrumentation 入口，验收真实十八键全拼，不增加清单项数。 */
final class EighteenKeyQuanpinDeviceChecks {
    private final DeviceSmoke t;
    private File preferences;
    private long revision;
    private JSONObject configured;

    private EighteenKeyQuanpinDeviceChecks(DeviceSmoke test) { t = test; }

    static void run(DeviceSmoke test) throws Exception {
        new EighteenKeyQuanpinDeviceChecks(test).run();
    }

    private void run() throws Exception {
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = json(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("合成测试目录越界");
        preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        revision = original == null ? 0 : json(preferences).getLong("revision");
        configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("learning", false).put("cloud_candidates", false)
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject localModes = configured.optJSONObject("local_modes");
        if (localModes != null) localModes.put("unicode", true).put("shift_letter_entry", true);
        try {
            t.stage = "eighteen quanpin baseline";
            save();
            t.restartIme();
            chooseLayout("26 键", "18 键");
            assertStored("eighteen_key");
            editor(false);
            t.await(group("Q"));
            t.await(group("W E"));
            t.await(group("L"));
            t.stage = "eighteen quanpin grouped nihao";
            groups("B N", "I O", "H", "A", "I O");
            t.await(t.field("msime-test-plain").and(EighteenKeyQuanpinDeviceChecks::empty));
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(n -> DeviceSmoke.equalsText("你好", n.getText())));

            t.stage = "eighteen quanpin reading lock and undo";
            groups("B N", "I O");
            t.await(t.described("18 键拼音选择"));
            t.await(t.described("选择拼音 ni"));
            t.await(t.described("选择拼音 bi"));
            t.tap(t.described("选择拼音 ni"));
            t.await(reading("ni"));
            t.tap(t.key("⌫"));
            t.await(t.described("选择拼音 bi"));
            t.tap(t.key("展开").and(AccessibilityNodeInfo::isEnabled));
            t.await(t.described("九键候选面板"));
            t.tap(t.described("收起候选面板"));
            t.tap(t.described("选择拼音 ni"));
            t.tap(t.described("按键 分词，在这里断开音节"));
            groups("H", "A", "I O");
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(n -> DeviceSmoke.equalsText("你好你好", n.getText())));

            t.stage = "eighteen quanpin English and number roundtrip";
            t.tap(t.key("中"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            t.tap(t.key("英"));
            t.await(group("W E"));
            t.tap(t.key("123"));
            t.await(t.key("1").and(AccessibilityNodeInfo::isClickable));
            t.tap(t.described("切换到字母键盘"));
            t.await(group("W E"));

            t.stage = "eighteen quanpin hardware letters stay precise";
            t.shellDiscard("input text nihao");
            t.tap(candidate("你好"));
            t.await(t.field("msime-test-plain").and(n -> DeviceSmoke.equalsText("你好你好你好", n.getText())));

            t.stage = "eighteen quanpin long press selects literal letter";
            if (!t.await(group("W E")).performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
                throw new AssertionError("十八键长按未打开字母选择");
            t.await(t.described("输入 e"));
            t.tap(t.described("输入 w"));
            t.await(t.field("msime-test-plain").and(n -> DeviceSmoke.equalsText("你好你好你好w", n.getText())));
            if (t.await(group("L")).isLongClickable())
                throw new AssertionError("单字母 L 不应提供长按字母菜单");

            t.stage = "eighteen quanpin local mode uses precise keyboard";
            t.shellDiscard("input text U42");
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            t.stage = "eighteen quanpin backspace returns from local mode";
            for (int index = 0; index < 3; index++) t.tap(t.key("⌫"));
            t.await(group("W E"));

            t.stage = "eighteen quanpin password and URL fallback";
            t.tap(t.field("msime-test-password"));
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            editor(true);
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
            editor(false);
            t.await(group("W E"));
            t.stage = "eighteen quanpin restart preserves selection";
            t.restartIme();
            editor(false);
            t.await(group("W E"));
            assertStored("eighteen_key");

            t.stage = "eighteen quanpin actual key geometry";
            geometries(root);
            chooseLayout("18 键", "14 键");
            assertStored("fourteen_key");
            editor(false);
            t.await(t.described("按键 Q W"));
            chooseLayout("14 键", "26 键");
            assertStored("twenty_six_key");
            editor(false);
            t.await(t.key("q").and(AccessibilityNodeInfo::isClickable));
        } catch (Exception | AssertionError failure) {
            t.shellDiscard("screencap -p /data/local/tmp/msime-8892-failure.png");
            throw failure;
        } finally {
            t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else t.publish(preferences, original);
        }
    }

    private void geometries(File root) throws Exception {
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        JSONObject settings = original == null ? new JSONObject().put("version", 1)
            .put("settings", new JSONObject()) : json(local);
        JSONObject values = settings.getJSONObject("settings");
        String automatic = shellText("settings get system accelerometer_rotation").trim();
        String rotation = shellText("settings get system user_rotation").trim();
        Matcher override = Pattern.compile("Override size: ([0-9]+x[0-9]+)").matcher(shellText("wm size"));
        String size = override.find() ? override.group(1) : "reset";
        try {
            t.shellDiscard("settings put system accelerometer_rotation 0");
            for (String mode : new String[] {"portrait", "narrow", "left", "right", "floating", "landscape"}) {
                t.stage = "eighteen quanpin geometry " + mode;
                boolean landscape = mode.equals("landscape");
                t.shellDiscard("wm size " + (mode.equals("narrow") ? "720x1280" : size));
                t.shellDiscard("settings put system user_rotation " + (landscape ? "1" : "0"));
                values.put("platform.android.one_handed", mode.equals("left") || mode.equals("right") ? mode : "off")
                    .put("platform.android.floating_keyboard", mode.equals("floating"));
                t.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
                t.restartIme();
                editor(false);
                long deadline = SystemClock.uptimeMillis() + 15000;
                while ((t.getTargetContext().getResources().getConfiguration().orientation
                        == Configuration.ORIENTATION_LANDSCAPE) != landscape) {
                    if (SystemClock.uptimeMillis() >= deadline)
                        throw new AssertionError("合成编辑器方向未稳定");
                    SystemClock.sleep(100);
                }
                t.await(group("W E"));
                geometry();
                t.shellDiscard("screencap -p /data/local/tmp/msime-8892-" + mode + ".png");
            }
        } finally {
            t.shellDiscard("wm size " + size);
            restoreSystem("user_rotation", rotation);
            restoreSystem("accelerometer_rotation", automatic);
            if (original == null) Files.deleteIfExists(local.toPath());
            else t.publish(local, original);
            t.restartIme();
        }
    }

    private void restoreSystem(String key, String value) throws Exception {
        t.shellDiscard(value.matches("[0-9]+") ? "settings put system " + key + " " + value
            : "settings delete system " + key);
    }

    private String shellText(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(t.automation.executeShellCommand(command));
                var output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[1024];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }

    private void geometry() {
        Rect first = bounds(group("Q")), last = bounds(group("P"));
        Rect middle = bounds(group("A")), middleLast = bounds(group("L"));
        Rect bottom = bounds(group("Z"));
        int tolerance = 2;
        if (Math.abs(first.width() - middle.width()) > tolerance
                || Math.abs(first.width() - bottom.width()) > tolerance)
            throw new AssertionError("十八键三排的字母键宽度不一致：" + first + ", " + middle + ", " + bottom);
        if (Math.abs(first.left + last.right - middle.left - middleLast.right) > tolerance)
            throw new AssertionError("十八键第二排没有居中");
        for (String face : new String[] {"Q", "W E", "R T", "Y", "U", "I O", "P", "A", "S D",
                "F G", "H", "J K", "L", "Z", "X C", "V", "B N", "M"}) {
            Rect key = bounds(group(face));
            if (key.width() <= 0 || key.height() <= 0)
                throw new AssertionError("十八键出现不可点击的零尺寸键");
        }
    }

    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect result = new Rect();
        t.awaitStableBounds(match).getBoundsInScreen(result);
        return result;
    }

    private void chooseLayout(String before, String after) throws Exception {
        t.stage = "native quanpin layout open settings " + before;
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        t.tap(t.key("中文键盘"));
        t.stage = "native quanpin layout select " + after;
        t.tap(t.key(after));
    }

    private void editor(boolean url) throws Exception {
        t.shellDiscard("am start -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
            + " --ez msime_test_url " + url);
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.dropStaleNodes();
    }

    private void groups(String... faces) throws Exception {
        for (String face : faces) t.tap(group(face));
    }

    private Predicate<AccessibilityNodeInfo> group(String face) {
        return t.described((face.length() == 1 ? "字母 " : "按键 ") + face);
    }

    private Predicate<AccessibilityNodeInfo> reading(String text) {
        return t.described("编辑拼写").and(n -> DeviceSmoke.equalsText(text, n.getText()));
    }

    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int colon = description.indexOf('：');
            if (!description.startsWith("候选 ") || colon < 0) return false;
            String value = description.substring(colon + 1);
            return (value.contains("；") ? value.substring(0, value.indexOf('；')) : value).equals(text);
        };
    }

    private void save() throws Exception {
        if (preferences.exists()) revision = Math.max(revision, json(preferences).getLong("revision"));
        t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", configured).toString().getBytes(StandardCharsets.UTF_8));
    }

    private void assertStored(String layout) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            JSONObject stored = json(preferences).getJSONObject("preferences");
            if (layout.equals(stored.optString("touch_keyboard_layout"))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("原生设置没有持久化所选全拼布局");
    }

    private static JSONObject json(File path) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(path.toPath()), StandardCharsets.UTF_8));
    }

    private static boolean empty(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }
}
