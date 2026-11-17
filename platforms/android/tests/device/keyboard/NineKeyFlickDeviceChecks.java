package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 从系统输入法入口回放真实触摸，验证字母、无动作方向、取消、长按数字和共享阈值。 */
final class NineKeyFlickDeviceChecks {
    private final DeviceSmoke t;
    private String expected = "";

    NineKeyFlickDeviceChecks(DeviceSmoke test) { t = test; }

    void run(boolean before) throws Exception {
        File preferences = t.sharedPreferencesFile();
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(t.getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "nine_key")
            .put("touch_nine_key_flick_threshold", 24).put("touch_spelling_position", "candidate_bar")
            .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
            .put("character_width", "halfwidth").put("learning", false).put("cloud_candidates", false)
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("nine_key")).put("selected", "nine_key"));
        try {
            t.stage = "prepare nine-key flick";
            t.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                .put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
            t.restartIme();
            editor();
            screenshot(before ? "before-keyboard" : "after-keyboard");
            setting(before);
            if (before) return;
            editor();
            String[] faces = {"ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ"};
            for (String face : faces) {
                for (int direction = 0; direction < 3; direction++) {
                    t.stage = "flick " + face + " direction " + direction;
                    gesture(face, direction == 1 ? -36 : direction == 2 ? 36 : 0, direction == 0 ? -36 : 0, false);
                    expected += Character.toLowerCase(face.charAt(direction));
                    expect();
                }
            }
            t.stage = "down flick has no tap fallback";
            gesture("ABC", 0, 25, false);
            expect();
            t.stage = "one key has no flick action";
            gesture("@#", 0, -36, false);
            expect();
            t.stage = "cancelled flick has no output";
            gesture("ABC", -36, 0, true);
            expect();
            t.stage = "long press retains digit choice";
            hold("ABC");
            t.tap(t.key("2"));
            expected += "2";
            expect();
            t.stage = "minimum shared threshold changes the next touch";
            t.putSharedPreference("touch_nine_key_flick_threshold", 14);
            t.await(t.key("ABC"));
            SystemClock.sleep(1300);
            gesture("ABC", 0, -20, false);
            expected += "a";
            expect();
            t.stage = "maximum threshold preserves a short tap";
            t.putSharedPreference("touch_nine_key_flick_threshold", 48);
            SystemClock.sleep(1300);
            gesture("ABC", 0, -20, false);
            t.await(t.described("编辑拼写"));
            t.tap(t.key("⌫"));
            expect();
            screenshot("after-result");
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else t.publish(preferences, original);
        }
    }

    private void editor() throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
        t.tap(t.field("msime-test-plain"));
        t.selectChineseKeyboard();
        t.await(t.key("ABC"));
    }

    private void setting(boolean before) throws Exception {
        t.stage = before ? "capture previous nine-key setting" : "capture shared flick threshold setting";
        Bundle args = new Bundle();
        args.putString("row", before ? "九键滑动输入数字" : "九键 flick 阈值");
        t.getTargetContext().startActivity(new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra("msime_open_page", "KEYBOARD_OPTIONS").putExtra("msime_page_args", args));
        AccessibilityNodeInfo ready = t.await(node -> DeviceSmoke.equalsText("临时说明（首次运行）", node.getText())
            || DeviceSmoke.equalsText(before ? "九键滑动输入数字" : "九键 flick 阈值", node.getText()));
        if (DeviceSmoke.equalsText("临时说明（首次运行）", ready.getText())) t.tap(t.key("知道了"));
        t.await(node -> DeviceSmoke.equalsText(before ? "九键滑动输入数字" : "九键 flick 阈值", node.getText()));
        screenshot(before ? "before-setting" : "after-setting");
    }

    private void gesture(String face, float dx, float dy, boolean cancel) {
        Rect bounds = new Rect();
        t.await(t.key(face)).getBoundsInScreen(bounds);
        float density = t.getTargetContext().getResources().getDisplayMetrics().density;
        long down = SystemClock.uptimeMillis();
        event(down, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        for (int step = 1; step <= 5; step++) {
            SystemClock.sleep(12);
            event(down, MotionEvent.ACTION_MOVE, bounds.centerX() + dx * density * step / 5,
                bounds.centerY() + dy * density * step / 5);
        }
        event(down, cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP,
            bounds.centerX() + dx * density, bounds.centerY() + dy * density);
    }

    private void hold(String face) {
        Rect bounds = new Rect();
        t.await(t.key(face)).getBoundsInScreen(bounds);
        long down = SystemClock.uptimeMillis();
        event(down, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        t.await(t.key("2"));
        event(down, MotionEvent.ACTION_UP, bounds.centerX(), bounds.centerY());
    }

    private void event(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.getUiAutomation().injectInputEvent(event, true)) throw new AssertionError("合成触摸注入失败");
        } finally { event.recycle(); }
    }

    private void expect() {
        t.await(t.field("msime-test-plain").and(node -> expected.isEmpty()
            ? node.isShowingHintText() || node.getText() == null || node.getText().length() == 0
            : DeviceSmoke.equalsText(expected, node.getText())));
    }

    private void screenshot(String suffix) throws Exception {
        t.shellDiscard("screencap -p /data/local/tmp/msime-flick-" + suffix + ".png");
    }
}
