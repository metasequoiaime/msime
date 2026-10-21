package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在真实 Android 键盘上验收九键主键面和 123 页叹号的上滑数字与长按角标。 */
public final class SuperscriptShortcutDeviceSmoke extends DeviceSmoke {
    private static final String PLAIN = "msime-test-plain";
    private static final float SWIPE_DP = 28f;

    @Override protected String successDescription() {
        return "nine-key and 123-page exclamation shortcuts preserve taps and type zero and superscript/subscript zero";
    }

    @Override protected void runChecks() throws Exception {
        checkNineKey();
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        stage = "focus editor";
        tap(field(PLAIN));
        await(field(PLAIN).and(AccessibilityNodeInfo::isFocused));
        stage = "open number layer";
        tap(described("切换到数字和符号"));
        // 中文数字入口默认显示九宫格，叹号仍在可达的扩展符号页。
        tap(key("#+="));
        AccessibilityNodeInfo exclamation = await(describedPrefix("！；上滑输入 0；长按选择上标"));

        stage = "long press superscript zero";
        if (!exclamation.isLongClickable()
                || !exclamation.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Exclamation key did not expose its long-press menu");
        tap(described("输入 ⁰"));
        await(field(PLAIN).and(node -> equalsText("⁰", node.getText())));

        stage = "swipe up ordinary zero";
        swipeUp(describedPrefix("！；上滑输入 0；长按选择上标"));
        await(field(PLAIN).and(node -> equalsText("⁰0", node.getText())));

        stage = "long press subscript zero";
        exclamation = await(describedPrefix("！；上滑输入 0；长按选择上标"));
        if (!exclamation.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Exclamation key long press did not reopen");
        tap(described("输入 ₀"));
        await(field(PLAIN).and(node -> equalsText("⁰0₀", node.getText())));
    }

    private void checkNineKey() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original,
            StandardCharsets.UTF_8)).getLong("revision");
        JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "nine_key")
            .put("ime_mode_scope", "app").put("default_ime_mode", "chinese")
            .put("touch_keyboard_schemes", new JSONObject().put("enabled",
                new JSONArray().put("quanpin").put("nine_key")).put("selected", "nine_key"));
        try {
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field(PLAIN));
            await(key("ABC"));
            stage = "nine-key main exclamation exposes shortcuts";
            if (!await(key("！")).isLongClickable())
                throw new AssertionError("Nine-key main exclamation has no long-press shortcut");
            stage = "nine-key tap still commits exclamation";
            tap(key("！"));
            await(field(PLAIN).and(node -> equalsText("！", node.getText())));
            stage = "nine-key swipe up commits zero";
            swipeUp(key("！"));
            await(field(PLAIN).and(node -> equalsText("！0", node.getText())));
            stage = "nine-key physical hold offers superscript zero";
            hold(key("！"));
            tap(described("输入 ⁰"));
            await(field(PLAIN).and(node -> equalsText("！0⁰", node.getText())));
            stage = "nine-key physical hold offers subscript zero";
            hold(key("！"));
            tap(described("输入 ₀"));
            await(field(PLAIN).and(node -> equalsText("！0⁰₀", node.getText())));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }

    private void hold(Predicate<AccessibilityNodeInfo> match) throws Exception {
        Rect bounds = new Rect();
        awaitStableBounds(match).getBoundsInScreen(bounds);
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(down, true)) throw new AssertionError("Long-press down rejected");
            SystemClock.sleep(650);
        } finally {
            down.recycle();
            MotionEvent up = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP,
                bounds.centerX(), bounds.centerY(), 0);
            up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            try {
                if (!automation.injectInputEvent(up, true)) throw new AssertionError("Long-press up rejected");
            } finally { up.recycle(); }
        }
    }

    private void swipeUp(java.util.function.Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = await(match);
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        float x = bounds.centerX();
        float startY = bounds.centerY();
        float endY = startY - SWIPE_DP * getTargetContext().getResources().getDisplayMetrics().density;
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, startY, 0);
        MotionEvent move = MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_MOVE, x, endY, 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 80, MotionEvent.ACTION_UP, x, endY, 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, move, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(event, true))
                    throw new AssertionError("Superscript shortcut swipe injection failed");
            }
        } finally {
            down.recycle();
            move.recycle();
            up.recycle();
        }
        SystemClock.sleep(150);
    }
}
