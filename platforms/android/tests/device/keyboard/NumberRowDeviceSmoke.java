package app.msime.android.test;

import android.graphics.Rect;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 数字行（#6022）的设备验收：打开本地设置后 26 键字母上方出现 1–0；空闲时点数字上屏数字；组字中点 Engine 不收的 0 先按首选结束组合再上屏，不顶掉组字区里的拼音；手机横屏时不画数字行。 */
public final class NumberRowDeviceSmoke extends AutomaticNumberRowDeviceSmoke {
    /** {@code AndroidLocalSettings.FILE_NAME} 与 {@code NUMBER_ROW}；设备套件不编译宿主的设置类，这里写同样的字面值。 */
    private static final String LOCAL_SETTINGS_FILE = "android-settings.json";
    private static final String NUMBER_ROW_SETTING = "platform.android.number_row";

    @Override protected String successDescription() {
        return "automatic context activation and IME reset, number row above the letters, idle digit commit, declined digit after the composition, five-row number/symbol pages and no extra row when disabled or in a landscape phone window";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            // 全拼断言不能继承上次设备套件留下的双拼或九键；结束后恢复原偏好。
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("ime_mode_scope", "app").put("default_ime_mode", "chinese")
                .put("character_width", "halfwidth")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                    .put("selected", "quanpin"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            super.runChecks();
            runManualNumberRowChecks();
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }

    private void runManualNumberRowChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        File localSettings = new File(new File(new File(root, "bootstrap"), "state"), LOCAL_SETTINGS_FILE);
        byte[] originalLocal = localSettings.exists() ? Files.readAllBytes(localSettings.toPath()) : null;
        // 旋转前的系统设置，结束时原样写回。
        String accelerometerRotation = null;
        String userRotation = null;
        try {
            stage = "enable number row";
            if (!localSettings.getParentFile().isDirectory() && !localSettings.getParentFile().mkdirs())
                throw new AssertionError("Local settings directory unavailable");
            publish(localSettings, new JSONObject().put("version", 1)
                .put("settings", new JSONObject().put(NUMBER_ROW_SETTING, true))
                .toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            if (findVisible(key("英")) != null) tap(key("英"));
            await(key("中"));

            stage = "number row above the letters";
            Rect q = bounds(key("q"));
            Rect one = bounds(digit("1"));
            Rect zero = bounds(digit("0"));
            if (one.bottom > q.top || Math.abs(one.centerY() - zero.centerY()) > 2)
                throw new AssertionError("Number row is not one row above the letters: " + one + " " + zero + " " + q);

            stage = "idle digit commit";
            tap(digit("5"));
            await(field("msime-test-plain").and(node -> equalsText("5", node.getText())));

            stage = "composition before a declined digit";
            for (String key : new String[] {"n", "i", "h", "a", "o"}) tap(key(key));
            await(field("msime-test-plain").and(node -> equalsText("5nihao", node.getText())));
            // 0 不是选词键，Engine 不收：先按首选结束组合，再上屏 0。原来 0 直接写进组字区，把 nihao 换成 0。
            stage = "declined digit after the composition";
            tap(digit("0"));
            await(field("msime-test-plain").and(node -> equalsText("5你好0", node.getText())));

            stage = "numeric pad retains the extra row and height";
            Rect letterTop = bounds(digit("1"));
            Rect letterBottom = bounds(key("123"));
            tap(key("123"));
            Rect extra = bounds(key("‰"));
            if (Math.abs(extra.top - letterTop.top) > 2 || Math.abs(extra.height() - letterTop.height()) > 2
                    || Math.abs(bounds(key("0")).bottom - letterBottom.bottom) > 2)
                throw new AssertionError("Number pad changed five-row height: " + letterTop + " " + extra);
            tap(key("‰"));
            await(field("msime-test-plain").and(node -> equalsText("5你好0‰", node.getText())));
            stage = "extended symbols retain the digit row";
            tap(key("#+="));
            Rect symbolsTop = bounds(digit("1"));
            if (Math.abs(symbolsTop.top - letterTop.top) > 2
                    || Math.abs(symbolsTop.height() - letterTop.height()) > 2)
                throw new AssertionError("Extended symbols changed extra-row geometry");
            tap(digit("7"));
            await(field("msime-test-plain").and(node -> equalsText("5你好0‰7", node.getText())));
            tap(key("123"));
            await(key("‰"));
            tap(described("切换到字母键盘"));
            await(key("q"));

            stage = "English number page adds useful symbols";
            tap(key("中"));
            await(key("英"));
            tap(key("123"));
            Rect bracket = bounds(key("["));
            if (Math.abs(bracket.top - letterTop.top) > 2)
                throw new AssertionError("English number page lost its fifth row");
            tap(key("["));
            await(field("msime-test-plain").and(node -> equalsText("5你好0‰7[", node.getText())));
            tap(key("ABC"));
            tap(key("英"));
            await(key("中"));

            // 手机横屏时窗口不到 480 dp 高（KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP），不画数字行，免得工具栏或底行被挤出窗口；转回竖屏数字行回来。平板横屏照画，所以只在最短边不到 480 dp 的设备上检查。
            if (getTargetContext().getResources().getConfiguration().smallestScreenWidthDp < 480) {
                stage = "landscape phone hides the number row";
                accelerometerRotation = shellOutput("settings get system accelerometer_rotation");
                userRotation = shellOutput("settings get system user_rotation");
                shellDiscard("settings put system accelerometer_rotation 0");
                shellDiscard("settings put system user_rotation 1");
                awaitEditorOrientation(true);
                tap(field("msime-test-plain"));
                awaitStableBounds(key("q"));
                SystemClock.sleep(800);
                if (findVisible(digit("1")) != null)
                    throw new AssertionError("Number row drawn in a landscape phone window; IME showed " + imeTexts());
                stage = "portrait brings the number row back";
                shellDiscard("settings put system user_rotation 0");
                awaitEditorOrientation(false);
                tap(field("msime-test-plain"));
                await(digit("1"));
            }
            stage = "disabled number row also removes the symbol extra row";
            publish(localSettings, new JSONObject().put("version", 1)
                .put("settings", new JSONObject().put(NUMBER_ROW_SETTING, false))
                .toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            awaitStableBounds(key("q"));
            if (findVisible(digit("1")) != null)
                throw new AssertionError("Disabled number row remains visible");
            tap(key("123"));
            awaitStableBounds(digit("1"));
            if (findVisible(key("‰")) != null)
                throw new AssertionError("Disabled number row still adds a symbol row");
        } finally {
            if (userRotation != null) shellDiscard("settings put system user_rotation " + settingValue(userRotation, "0"));
            if (accelerometerRotation != null)
                shellDiscard("settings put system accelerometer_rotation " + settingValue(accelerometerRotation, "1"));
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (originalLocal == null) Files.deleteIfExists(localSettings.toPath());
            else publish(localSettings, originalLocal);
            restartIme();
        }
    }

    /**
     * 改了屏幕方向后，等编辑器窗口按新方向排好、输入框位置稳定下来再去点。旋转过渡期间编辑器 Activity 正在重建，这时注入的点按没有窗口接收（logcat 里是 `ActivityRecordInputSink ... NO_INPUT_CHANNEL` 和 `Dropping event because no targets were found`），`tap` 报 Touch injection failed；CI 的 API 35 上就这样失败过一次。
     */
    private void awaitEditorOrientation(boolean landscape) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            automation.waitForIdle(500, 5000);
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (window.getType() != AccessibilityWindowInfo.TYPE_APPLICATION) continue;
                AccessibilityNodeInfo root = window.getRoot();
                if (root == null || !equalsText("app.msime.android.test", root.getPackageName())) continue;
                Rect bounds = new Rect();
                window.getBoundsInScreen(bounds);
                if (!bounds.isEmpty() && (bounds.width() > bounds.height()) == landscape) {
                    awaitStableBounds(field("msime-test-plain"));
                    return;
                }
            }
            SystemClock.sleep(250);
        }
        throw new AssertionError("Editor window never turned " + (landscape ? "landscape" : "portrait")
            + "; IME showed " + imeTexts());
    }

    private AccessibilityNodeInfo findVisible(Predicate<AccessibilityNodeInfo> match) {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo found = find(window.getRoot(), match);
            if (found != null) return found;
        }
        return null;
    }

    /** 数字行的键：只认按钮，候选、读音里的数字不算。 */
    private Predicate<AccessibilityNodeInfo> digit(String text) {
        return key(text).and(node -> equalsText("android.widget.Button", node.getClassName()));
    }

    /** 键盘弹出和重建键行时有动画，量到的是过渡位置；等位置连续两次读到一样再量。 */
    private Rect bounds(Predicate<AccessibilityNodeInfo> match) {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        return bounds;
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }


    private String shellOutput(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            // InputStream.readAllBytes 在 API 33 才可用；最低版本设备也要执行横屏验收。
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return new String(output.toByteArray(), StandardCharsets.UTF_8).trim();
        }
    }

    /** `settings get` 对没写过的键返回 `null`，这时写回系统默认值。 */
    private static String settingValue(String value, String fallback) {
        return value.matches("[0-9]+") ? value : fallback;
    }


}
