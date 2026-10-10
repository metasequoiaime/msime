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

/** 键盘高度调整与持久化（与 Apple 一致）、键区下的底部留白，以及底行与字母键同高的设备验收。 */
public final class KeyboardHeightDeviceSmoke extends DeviceSmoke {
    /** {@code AndroidLocalSettings.FILE_NAME} 与 {@code KEYBOARD_HEIGHT_ADJUSTMENT}；设备套件不编译宿主的设置类，这里写同样的字面值。 */
    private static final String LOCAL_SETTINGS_FILE = "android-settings.json";
    private static final String HEIGHT_SETTING = "platform.android.keyboard_height_adjustment";
    /** {@code AndroidLocalSettings.BOTTOM_BAR} 与 {@code BOTTOM_PADDING}，同样写字面值。 */
    private static final String BOTTOM_BAR_SETTING = "platform.android.bottom_bar";
    private static final String BOTTOM_PADDING_SETTING = "platform.android.bottom_padding";

    @Override protected String successDescription() {
        return "keyboard height live preview, composition preservation, persistence, restart, bottom padding and a bottom row as tall as the letter rows";
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

            // 不点「完成」就收起键盘：收起照「完成」保存，只有「取消」才放弃预览。110% 是 18 dp（KeyboardHeightPolicy.heightPercentToAdjustment）。
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

            // 底行与字母键同高（#6354、#6472）：默认、75% 和 130% 时空格键都和「n」一样高——底行和三行键一起均分高度调整，两者最多差 1 dp。默认高度时从「q」的上沿到空格的下沿是四行 52 dp 加三份行距（229 dp），与原来三行 56 dp 加 39 dp 的底栏键帽（228 dp）基本相同。同样关掉键盘底栏，排除手势导航 AVD 上的差异。
            stage = "bottom row matches the letter rows";
            float density = getTargetContext().getResources().getDisplayMetrics().density;
            int tolerance = Math.round(density) + 2;
            for (int adjustment : new int[] {0, -46, 55}) {
                Rect letter = settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false)
                    .put(HEIGHT_SETTING, adjustment));
                Rect space = keyBounds("空格");
                if (Math.abs(space.height() - letter.height()) > tolerance)
                    throw new AssertionError("Bottom row is not as tall as the letter rows at " + adjustment
                        + " dp: space " + space.height() + " vs n " + letter.height());
                if (adjustment == 0) {
                    int span = space.bottom - keyBounds("q").top;
                    int expected = Math.round(229 * density);
                    if (Math.abs(span - expected) > Math.round(2 * density) + 2)
                        throw new AssertionError("Default key area changed: " + span + " px from q to space, expected "
                            + expected);
                }
            }

            // 全拼 14 键：三行两字母键加 26 键的底行，键区总高与 26 键相同，底行也和字母键同高。
            stage = "fourteen-key keyboard is as tall as the 26 keys";
            settledKeyBounds(localSettings, new JSONObject().put(BOTTOM_BAR_SETTING, false));
            int twentySixSpan = keyBounds("空格").bottom - keyBounds("q").top;
            JSONObject fourteen = new JSONObject(snapshot.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "fourteen_key")
                .put("touch_keyboard_schemes", new JSONObject()
                    .put("enabled", new org.json.JSONArray().put("quanpin").put("fourteen_key"))
                    .put("selected", "fourteen_key"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 2)
                .put("preferences", fourteen).toString().getBytes(StandardCharsets.UTF_8));
            rebindInputMethod();
            openEditor();
            Rect pair = new Rect();
            awaitStableBounds(description("按键 Q W")).getBoundsInScreen(pair);
            Rect fourteenSpace = keyBounds("空格");
            int fourteenSpan = fourteenSpace.bottom - pair.top;
            if (Math.abs(fourteenSpan - twentySixSpan) > tolerance)
                throw new AssertionError("Fourteen-key area is " + fourteenSpan + " px, the 26 keys " + twentySixSpan);
            if (Math.abs(fourteenSpace.height() - pair.height()) > tolerance)
                throw new AssertionError("Fourteen-key bottom row is not as tall as its keys: space "
                    + fourteenSpace.height() + " vs QW " + pair.height());
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
