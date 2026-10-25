package app.msime.android.test;

import android.content.res.Configuration;
import android.graphics.Rect;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import org.json.JSONObject;

/** 专用模拟器中的 18 键实际触控范围；恢复屏幕配置和本地设置，不增加 instrumentation 入口。 */
final class EighteenKeyGeometryDeviceChecks {
    private static final String[][] ROWS = {
        {"字母 Q", "按键 W E", "按键 R T", "字母 Y", "字母 U", "按键 I O", "字母 P"},
        {"字母 A", "按键 S D", "按键 F G", "字母 H", "按键 J K", "按键 L 分号"},
        {"字母 Z", "按键 X C", "字母 V", "按键 B N", "字母 M"}
    };
    private final DeviceSmoke t;
    private final File local;
    private final JSONObject document;

    private EighteenKeyGeometryDeviceChecks(DeviceSmoke test, File local, JSONObject document) {
        t = test;
        this.local = local;
        this.document = document;
    }

    static void run(DeviceSmoke test) throws Exception {
        File local = new File(test.getTargetContext().getFilesDir(), "bootstrap/state/android-settings.json");
        byte[] original = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        JSONObject document = original == null
            ? new JSONObject().put("version", 1).put("settings", new JSONObject())
            : new JSONObject(new String(original, StandardCharsets.UTF_8));
        EighteenKeyGeometryDeviceChecks checks = new EighteenKeyGeometryDeviceChecks(test, local, document);
        String size = checks.override("wm size", "size", "[0-9]+x[0-9]+");
        String density = checks.override("wm density", "density", "[0-9]+");
        String rotation = checks.shell("settings get system user_rotation").trim();
        String automatic = checks.shell("settings get system accelerometer_rotation").trim();
        try {
            test.shellDiscard("settings put system accelerometer_rotation 0");
            test.shellDiscard("settings put system user_rotation 0");
            test.shellDiscard("wm density 240");
            checks.mode(false, "off");
            checks.resize(480, 1200);
            checks.checkAndCapture(480, 1200, "narrow");
            checks.resize(1200, 720);
            checks.checkAndCapture(1200, 720, "landscape");
            checks.resize(720, 1440);
            Rect full = checks.checkAndCapture(720, 1440, "portrait");
            checks.mode(true, "off");
            Rect floating = checks.checkAndCapture(720, 1440, "floating");
            if (floating.width() >= full.width()) throw new AssertionError("浮动键盘没有缩窄");
            checks.mode(false, "left");
            Rect left = checks.checkAndCapture(720, 1440, "one-hand-left");
            checks.mode(false, "right");
            Rect right = checks.checkAndCapture(720, 1440, "one-hand-right");
            if (left.width() >= full.width() || right.width() >= full.width() || right.left <= left.left)
                throw new AssertionError("单手键盘没有缩窄或未按左右侧对齐");
        } finally {
            if (original == null) Files.deleteIfExists(local.toPath()); else test.publish(local, original);
            test.shellDiscard("wm size " + size);
            test.shellDiscard("wm density " + density);
            checks.restoreSystem("user_rotation", rotation);
            checks.restoreSystem("accelerometer_rotation", automatic);
            test.restartIme();
        }
    }

    private void mode(boolean floating, String hand) throws Exception {
        document.getJSONObject("settings").put("platform.android.floating_keyboard", floating)
            .put("platform.android.one_handed", hand).put("platform.android.split_keyboard", false);
        t.publish(local, document.toString().getBytes(StandardCharsets.UTF_8));
        t.restartIme();
    }

    private void resize(int width, int height) throws Exception {
        t.shellDiscard("wm size " + width + "x" + height);
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            Configuration configuration = t.getTargetContext().getResources().getConfiguration();
            if ((configuration.orientation == Configuration.ORIENTATION_LANDSCAPE) == (width > height)) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("18 键验收的屏幕方向未更新");
    }

    private Rect checkAndCapture(int width, int height, String name) throws Exception {
        t.stage = "eighteen-key geometry " + name;
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.await(t.field("msime-test-plain").and(AccessibilityNodeInfo::isFocused));
        t.dropStaleNodes();
        Rect screen = new Rect(0, 0, width, height);
        Rect area = new Rect();
        List<Rect> keys = new ArrayList<>();
        int minWidth = Integer.MAX_VALUE, maxWidth = 0;
        for (String[] row : ROWS) {
            Rect first = null, last = null;
            for (String description : row) {
                AccessibilityNodeInfo node = t.awaitStableBounds(t.described(description));
                Rect bounds = new Rect();
                node.getBoundsInScreen(bounds);
                if (!node.isClickable() || !node.isVisibleToUser() || bounds.isEmpty() || !screen.contains(bounds))
                    throw new AssertionError("18 键不可点击或超出屏幕：" + description + " " + bounds);
                for (Rect previous : keys) if (Rect.intersects(previous, bounds))
                    throw new AssertionError("18 键触控范围重叠：" + previous + " " + bounds);
                if (first == null) first = bounds;
                else if (bounds.left < last.right || Math.abs(bounds.top - first.top) > 2)
                    throw new AssertionError("18 键没有按三行顺序排列");
                last = bounds;
                minWidth = Math.min(minWidth, bounds.width());
                maxWidth = Math.max(maxWidth, bounds.width());
                keys.add(bounds);
                area.union(bounds);
            }
        }
        if (maxWidth - minWidth > 2)
            throw new AssertionError("18 键字母组没有等宽：" + minWidth + ".." + maxWidth + " " + keys);
        Rect first = keys.get(0), last = keys.get(6), a = keys.get(7), l = keys.get(12);
        if (Math.abs(first.left + last.right - a.left - l.right) > 4)
            throw new AssertionError("18 键第二排没有居中");
        t.shellDiscard("screencap -p /data/local/tmp/msime-9124-" + name + ".png");
        return area;
    }

    private String override(String command, String name, String value) throws Exception {
        var match = Pattern.compile("Override " + name + ": (" + value + ")").matcher(shell(command));
        return match.find() ? match.group(1) : "reset";
    }

    private String shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(t.automation.executeShellCommand(command))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    private void restoreSystem(String key, String value) throws Exception {
        t.shellDiscard(value.matches("[0-9]+") ? "settings put system " + key + " " + value
            : "settings delete system " + key);
    }
}
