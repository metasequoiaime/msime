package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** Device acceptance for Apple-compatible keyboard height adjustment and persistence, and for the bottom padding below the keys and the tall bottom row. */
public final class KeyboardHeightDeviceSmoke extends DeviceSmoke {
    /** {@code AndroidLocalSettings.FILE_NAME} 与 {@code KEYBOARD_HEIGHT_ADJUSTMENT}；设备套件不编译宿主的设置类，这里写同样的字面值。 */
    private static final String LOCAL_SETTINGS_FILE = "android-settings.json";
    private static final String HEIGHT_SETTING = "platform.android.keyboard_height_adjustment";
    /** {@code AndroidLocalSettings.BOTTOM_BAR}、{@code BOTTOM_PADDING} 与 {@code TALL_BOTTOM_ROW}，同样写字面值。 */
    private static final String BOTTOM_BAR_SETTING = "platform.android.bottom_bar";
    private static final String BOTTOM_PADDING_SETTING = "platform.android.bottom_padding";
    private static final String TALL_BOTTOM_ROW_SETTING = "platform.android.tall_bottom_row";

    @Override protected String successDescription() {
        return "keyboard height live preview, composition preservation, persistence, restart, bottom padding and tall bottom row";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the preview sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject snapshot = new JSONObject().put("format_version", 1).put("revision", revision + 1)
            .put("preferences", new JSONObject(options.getJSONObject("preferences").toString())
                .put("touch_keyboard_height_adjustment", 0));
        // 设计范围的键盘高度（75%..160%）存在 Android 本地设置里，不在共享偏好里。
        File localSettings = new File(new File(new File(root, "bootstrap"), "state"), LOCAL_SETTINGS_FILE);
        byte[] originalLocal = localSettings.exists() ? Files.readAllBytes(localSettings.toPath()) : null;
        JSONObject localBaseline = new JSONObject().put("version", 1).put("settings", new JSONObject());
        try {
            stage = "baseline preferences";
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            if (!localSettings.getParentFile().isDirectory() && !localSettings.getParentFile().mkdirs())
                throw new AssertionError("Local settings directory unavailable");
            publish(localSettings, localBaseline.toString().getBytes(StandardCharsets.UTF_8));
            rebindInputMethod();
            openEditor();
            stage = "baseline keyboard height";
            int standard = keyHeight("n");

            // 键盘高度 is a function-panel tile; it turns the toolbar row into the inline height bar, which previews while it is walked and saves only on 完成. The panel is reachable only from an idle keyboard, so each visit starts from the brand key.
            stage = "open inline height";
            openHeightBar();
            stage = "increase keyboard height";
            setHeight(130);
            tap(key("完成"));
            awaitHeightSetting(localSettings, 55);
            int tall = keyHeight("n");
            int minimumDelta = Math.round(8 * getTargetContext()
                .getResources().getDisplayMetrics().density);
            if (tall < standard + minimumDelta)
                throw new AssertionError("Taller keyboard did not enlarge key faces: "
                    + standard + " -> " + tall + ", expected delta " + minimumDelta);

            stage = "decrease keyboard height";
            openHeightBar();
            setHeight(75);
            tap(key("完成"));
            awaitHeightSetting(localSettings, -46);
            int shortHeight = keyHeight("n");
            if (shortHeight >= standard)
                throw new AssertionError("Shorter keyboard did not shrink key faces: "
                    + standard + " -> " + shortHeight);

            stage = "cancel keeps the saved height";
            openHeightBar();
            setHeight(120);
            tap(key("取消"));
            await(key("n").and(AccessibilityNodeInfo::isClickable));
            if (Math.abs(keyHeight("n") - shortHeight) > 2)
                throw new AssertionError("Cancelled preview was not reverted");

            // 不点「完成」就收起键盘：收起照「完成」保存，只有「取消」才放弃预览。110% 是 18 dp（KeyboardGeometry.heightPercentToAdjustment）。
            stage = "hiding mid-adjustment keeps the height";
            openHeightBar();
            setHeight(110);
            shell("input keyevent KEYCODE_BACK");
            awaitHeightSetting(localSettings, 18);
            openEditor();
            int keptHeight = keyHeight("n");
            if (keptHeight <= shortHeight + 2)
                throw new AssertionError("Height adjusted before hiding was not kept: "
                    + shortHeight + " -> " + keptHeight);

            stage = "reset to 100 percent";
            openHeightBar();
            tap(key("重置"));
            if (Math.round(await(heightSlider()).getRangeInfo().getCurrent()) != 100)
                throw new AssertionError("重置 did not return the bar to 100%");
            tap(key("完成"));
            awaitHeightSetting(localSettings, 0);
            int resetHeight = keyHeight("n");
            if (Math.abs(resetHeight - standard) > 2)
                throw new AssertionError("Reset keyboard height did not return to default: "
                    + standard + " -> " + resetHeight);

            stage = "height survives input method restart";
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            rebindInputMethod();
            openEditor();
            int restarted = keyHeight("n");
            if (Math.abs(restarted - resetHeight) > 2)
                throw new AssertionError("Persisted height changed after restart");

            // 底部留白（#6392）：键区下面垫 40 dp 空白，底行键整体抬高同样的距离，键本身不变高。两次都关掉键盘底栏：底栏画着时留白不叠加，手势导航的 AVD 上基线会不同。
            stage = "bottom padding lifts the bottom row";
            Rect unpadded = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
            Rect padded = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                .put(BOTTOM_PADDING_SETTING, true));
            int lift = unpadded.bottom - padded.bottom;
            int expectedLift = Math.round(40 * getTargetContext().getResources().getDisplayMetrics().density);
            if (Math.abs(lift - expectedLift) > 2)
                throw new AssertionError("Bottom padding lifted the keys by " + lift + " px, expected " + expectedLift);
            if (Math.abs(padded.height() - unpadded.height()) > 2)
                throw new AssertionError("Bottom padding changed the key height: "
                    + unpadded.height() + " -> " + padded.height());

            // 加高底行（#6354）：关着时底栏的键帽比字母键矮一截（默认 39 dp 对 56 dp），打开后空格键和「n」同高，字母键本身不变。同样关掉键盘底栏，排除手势导航 AVD 上的差异。
            stage = "tall bottom row matches the key rows";
            Rect letterDefault = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
            Rect spaceDefault = keyBounds("空格");
            Rect letterTall = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                .put(TALL_BOTTOM_ROW_SETTING, true));
            Rect spaceTall = keyBounds("空格");
            int shortfall = Math.round(8 * getTargetContext().getResources().getDisplayMetrics().density);
            if (spaceDefault.height() > letterDefault.height() - shortfall)
                throw new AssertionError("Default bottom row was not shorter than the key rows: space "
                    + spaceDefault.height() + " vs n " + letterDefault.height());
            if (Math.abs(spaceTall.height() - letterTall.height()) > 2)
                throw new AssertionError("Tall bottom row did not match the key rows: space "
                    + spaceTall.height() + " vs n " + letterTall.height());
            if (Math.abs(letterTall.height() - letterDefault.height()) > 2)
                throw new AssertionError("Tall bottom row changed the letter key height: "
                    + letterDefault.height() + " -> " + letterTall.height());
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(localSettings.toPath());
            else publish(localSettings, originalLocal);
        }
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }

    private void rebindInputMethod() throws Exception {
        shell("ime disable app.msime.android/.MSIMEInputService");
        shell("ime enable app.msime.android/.MSIMEInputService");
        shell("ime set app.msime.android/.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    /** 写入这份本地设置、重新绑定输入法后，等键盘停稳再量「n」键在屏幕上的位置：键盘弹出有动画，动画中量到的位置是过渡值。 */
    private Rect settledKeyBounds(File localSettings, JSONObject settings) throws Exception {
        publish(localSettings, new JSONObject().put("version", 1).put("settings", settings).toString()
            .getBytes(StandardCharsets.UTF_8));
        rebindInputMethod();
        openEditor();
        Rect bounds = new Rect();
        awaitStableBounds(key("n")).getBoundsInScreen(bounds);
        return bounds;
    }

    /** 键盘已停稳时量一个键在屏幕上的位置。 */
    private Rect keyBounds(String label) {
        Rect bounds = new Rect();
        await(key(label)).getBoundsInScreen(bounds);
        return bounds;
    }

    private int keyHeight(String label) {
        // A letter key is described 字母 N, not 按键 n: 按键 is the symbol-key form, and the letter
        // is drawn in caps in Chinese mode. Match the key itself and leave both to their policies.
        AccessibilityNodeInfo node = await(key(label));
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds.height();
    }

    /** The inline height bar: described 键盘布局调整, with a 75–160 percent range and five-percent scroll steps. */
    private java.util.function.Predicate<AccessibilityNodeInfo> heightSlider() {
        return description("键盘布局调整").and(node -> node.getRangeInfo() != null);
    }

    private void openHeightBar() throws Exception {
        tap(key("更多").and(AccessibilityNodeInfo::isEnabled));
        await(toolPanel());
        tap(tool("键盘高度").and(AccessibilityNodeInfo::isEnabled));
        await(heightSlider());
    }

    private java.util.function.Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    /**
     * Walk the height to `target`.
     *
     * <p>The bar exposes only the scroll actions, five percent apiece, so there is no progress to set; the range it reports is what says where the walk has got to.
     */
    private void setHeight(int target) {
        for (int guard = 0; guard < 64; guard++) {
            AccessibilityNodeInfo slider = await(heightSlider());
            // The framework hands back cached node info; without this the range never moves.
            slider.refresh();
            int current = Math.round(slider.getRangeInfo().getCurrent());
            if (current == target) return;
            int action = current < target
                ? AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                : AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD;
            if (!slider.performAction(action))
                throw new AssertionError("Height accessibility action failed at " + current);
        }
        throw new AssertionError("Height adjustment never reached " + target);
    }

    private void awaitHeightSetting(File file, int expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            try {
                byte[] bytes = Files.readAllBytes(file.toPath());
                JSONObject current = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
                if (current.getJSONObject("settings").optInt(HEIGHT_SETTING, Integer.MIN_VALUE) == expected) return;
            } catch (java.io.IOException | RuntimeException | org.json.JSONException ignored) {
                // Atomic replacement can briefly expose no complete file to this polling read.
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Height setting was not saved");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try {
            output.write(contents);
            target.finishWrite(output);
        } catch (Exception error) {
            target.failWrite(output);
            throw error;
        }
    }
}
