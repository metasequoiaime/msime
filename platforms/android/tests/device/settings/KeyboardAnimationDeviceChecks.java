package app.msime.android.test;

import android.os.Build;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/** 在同一设置页反复打开真实动画面板，核对持久化值和唯一选中标记。 */
final class KeyboardAnimationDeviceChecks {
    private static final String KEY = "platform.android.key_animation";
    private final DeviceSmoke test;
    private File local;

    KeyboardAnimationDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        local = new File(test.getTargetContext().getFilesDir(), "bootstrap/state/android-settings.json");
        byte[] original = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        JSONObject settings = original == null ? new JSONObject().put("version", 1).put("settings", new JSONObject())
            : new JSONObject(new String(original, StandardCharsets.UTF_8));
        settings.getJSONObject("settings").put(KEY, "bounce");
        try {
            test.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            openSettings();
            openSheet();
            expectSelected("弹起");
            choose("涟漪", "ripple");
            test.stage = "animation reopen uses just-saved value on same page";
            openSheet();
            expectSelected("涟漪");
            choose("无", "none");
            openSheet();
            expectSelected("无");
            click("取消");
            test.stage = "animation cancellation preserves current value";
            openSheet();
            expectSelected("无");
            choose("浮起", "lift");
            test.stage = "animation page revisit uses persisted value";
            openSettings();
            openSheet();
            expectSelected("浮起");
            click("取消");
        } finally {
            test.shellDiscard("am start -W -n app.msime.android/.home.HomeActivity");
            if (original == null) Files.deleteIfExists(local.toPath());
            else test.publish(local, original);
        }
    }

    private void openSettings() throws Exception {
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        test.tap(node -> DeviceSmoke.equalsText("反馈", node.getText()));
        test.await(node -> DeviceSmoke.equalsText("按键动画", node.getText()));
    }

    private void openSheet() throws Exception {
        AccessibilityNodeInfo row = test.await(node -> DeviceSmoke.equalsText("按键动画", node.getText()));
        while (!row.isClickable() && row.getParent() != null) row = row.getParent();
        if (!row.performAction(AccessibilityNodeInfo.ACTION_CLICK)) throw new AssertionError("Animation row did not open");
        test.await(test.described("弹起"));
    }

    private void choose(String label, String value) throws Exception {
        click(label);
        long until = SystemClock.uptimeMillis() + 5000;
        do {
            JSONObject current = new JSONObject(new String(Files.readAllBytes(local.toPath()), StandardCharsets.UTF_8));
            if (value.equals(current.getJSONObject("settings").optString(KEY))) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("Animation selection was not saved");
    }

    private void expectSelected(String label) {
        int selected = 0;
        for (String option : new String[] {"弹起", "涟漪", "发光", "浮起", "无"}) {
            AccessibilityNodeInfo row = test.await(test.described(option));
            CharSequence state = Build.VERSION.SDK_INT >= 30 ? row.getStateDescription()
                : row.getExtras().getCharSequence("androidx.view.accessibility.AccessibilityNodeInfoCompat.STATE_DESCRIPTION_KEY");
            boolean checked = DeviceSmoke.equalsText("已选择", state);
            if (checked) selected++;
            if (checked != option.equals(label)) throw new AssertionError("Animation checkmark does not follow saved value");
        }
        if (selected != 1) throw new AssertionError("Animation panel needs exactly one checkmark");
    }

    private void click(String label) throws Exception { test.tap(test.described(label)); }
}
