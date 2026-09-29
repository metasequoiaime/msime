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

/** Device acceptance for Apple-compatible keyboard height adjustment and persistence. */
public final class KeyboardHeightDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "keyboard height live preview, composition preservation, persistence and restart";
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
        try {
            stage = "baseline preferences";
            publish(preferences, snapshot.toString().getBytes(StandardCharsets.UTF_8));
            rebindInputMethod();
            openEditor();
            stage = "baseline keyboard height";
            int standard = keyHeight("n");

            // 设置 lives in the shortcut bar, and the candidate strip takes that row while a
            // composition is open, so the panel is only reachable from an idle keyboard: resizing
            // mid-composition is not something this surface can be driven into.
            // 设置 is disabled while preferences are still loading and while a geometry save is
            // in flight, so each visit waits for it to be operable rather than clicking blind.
            stage = "open keyboard settings";
            tap(key("设置").and(AccessibilityNodeInfo::isEnabled));
            await(heightSlider());
            stage = "increase keyboard height";
            setHeight(48);
            long tallRevision = awaitHeightPreference(preferences, 48, revision + 2);
            stage = "return from tall setting";
            tap(description("返回键盘"));
            int tall = keyHeight("n");
            int minimumDelta = Math.round(12 * getTargetContext()
                .getResources().getDisplayMetrics().density);
            if (tall < standard + minimumDelta)
                throw new AssertionError("Positive adjustment did not enlarge key faces: "
                    + standard + " -> " + tall + ", expected delta " + minimumDelta);

            stage = "decrease keyboard height";
            tap(key("设置").and(AccessibilityNodeInfo::isEnabled));
            setHeight(-12);
            long shortRevision = awaitHeightPreference(preferences, -12, tallRevision + 1);
            tap(description("返回键盘"));
            int shortHeight = keyHeight("n");
            if (shortHeight >= standard)
                throw new AssertionError("Negative adjustment did not shrink key faces: "
                    + standard + " -> " + shortHeight);

            stage = "restore keyboard settings defaults";
            tap(key("设置").and(AccessibilityNodeInfo::isEnabled));
            // 恢复默认 is the button's face; its description is 恢复键盘布局默认值.
            await(description("恢复键盘布局默认值"));
            tap(description("恢复键盘布局默认值"));
            long resetRevision = awaitResetPreference(preferences, shortRevision + 1);
            if (resetRevision <= shortRevision)
                throw new AssertionError("Keyboard settings reset did not advance revision");
            if (await(heightSlider()).getRangeInfo().getCurrent() != 0f
                    || await(description("顶部语音入口")).isChecked())
                throw new AssertionError("Keyboard settings controls did not return to defaults");
            tap(description("返回键盘"));
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
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }

    private void rebindInputMethod() throws Exception {
        shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private int keyHeight(String label) {
        // A letter key is described 字母 N, not 按键 n: 按键 is the symbol-key form, and the letter
        // is drawn in caps in Chinese mode. Match the key itself and leave both to their policies.
        AccessibilityNodeInfo node = await(key(label));
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds.height();
    }

    /**
     * The transparent adjust layer, which reports itself as a SeekBar with the height range.
     *
     * <p>The old settings panel had a labelled 键盘高度 SeekBar; it is still in the tree but GONE
     * since the panel became the drag layer, so matching that label found an invisible view.
     */
    private java.util.function.Predicate<AccessibilityNodeInfo> heightSlider() {
        return describedPrefix("键盘布局调整").and(node -> node.getRangeInfo() != null);
    }

    private java.util.function.Predicate<AccessibilityNodeInfo> description(String value) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText(value, node.getContentDescription());
    }

    /**
     * Walk the height to `target`.
     *
     * <p>The adjust layer exposes only the scroll actions, two device-independent pixels apiece,
     * so there is no progress to set; the range it reports is what says where the walk has got to.
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

    private long awaitHeightPreference(File file, int expected, long minimumRevision)
            throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            byte[] bytes = Files.readAllBytes(file.toPath());
            try {
                JSONObject current = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
                if (current.getLong("revision") >= minimumRevision
                        && current.getJSONObject("preferences")
                            .getInt("touch_keyboard_height_adjustment") == expected) {
                    return current.getLong("revision");
                }
            } catch (RuntimeException ignored) {
                // Atomic replacement can briefly expose no complete snapshot to this polling read.
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Height preference was not saved");
    }

    private long awaitResetPreference(File file, long minimumRevision) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            byte[] bytes = Files.readAllBytes(file.toPath());
            try {
                JSONObject current = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
                JSONObject settings = current.getJSONObject("preferences");
                if (current.getLong("revision") >= minimumRevision
                        && !settings.has("touch_key_spacing_tenths")
                        && !settings.has("touch_row_spacing_tenths")
                        && !settings.has("touch_keyboard_height_adjustment")
                        && !settings.has("touch_voice_shortcut")) {
                    return current.getLong("revision");
                }
            } catch (RuntimeException ignored) {
                // Atomic replacement can briefly expose no complete snapshot to this polling read.
            }
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Keyboard settings reset was not saved");
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
