package app.msime.android.test;

import android.content.Context;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.ContextThemeWrapper;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用真实触摸检查候选条、完整网格与九键三栏选词后不必再按其他键就能刷新。 */
public final class CandidatePanelDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "touch candidate release redraws the strip, complete grid and nine-key panel";
    }

    @Override protected void runChecks() throws Exception {
        stage = "pressed release cancellation and repeated key guard";
        Throwable[] failure = {null};
        runOnMainSync(() -> {
            try { checkReleaseLifecycle(); }
            catch (Throwable error) { failure[0] = error; }
        });
        if (failure[0] != null) throw new AssertionError("松开状态回调不符合约定：" + failure[0], failure[0]);
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
            getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (boolean nineKey : new boolean[] {false, true}) {
                String layout = nineKey ? "nine_key" : "quanpin";
                JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("touch_keyboard_layout", nineKey ? "nine_key" : "twenty_six_key")
                    .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
                    .put("cloud_candidates", false).put("learning", false)
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject()
                        .put("enabled", new JSONArray().put("quanpin").put("nine_key"))
                        .put("selected", layout));
                stage = layout + " preferences";
                publish(preferences, new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
                if (!modes.edit().clear().commit()) throw new AssertionError("输入模式准备失败");
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                stage = layout + " strip composition";
                compose(nineKey);
                String first = candidateText(await(candidate(1)));
                stage = layout + " strip touch commit";
                touch(candidate(1));
                await(field("msime-test-plain").and(node -> equalsText(first, node.getText())));
                stage = layout + " strip released candidate redraw";
                awaitGone(describedPrefix("候选 "));

                stage = layout + " complete panel composition";
                compose(nineKey);
                tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
                await(completeCount());
                if (nineKey) await(described("九键候选面板"));
                AccessibilityNodeInfo later = await(candidate(10));
                if (later.getText() == null || later.getText().toString().startsWith("10. ")
                        || !later.isLongClickable())
                    throw new AssertionError("展开候选应隐藏序号并保留长按菜单");
                String expected = first + candidateText(later);
                stage = layout + " complete panel touch commit";
                touch(candidate(10));
                await(field("msime-test-plain").and(node -> equalsText(expected, node.getText())));
                stage = layout + " complete panel released candidate redraw";
                awaitGone(describedPrefix("完整候选列表；"));
                awaitGone(described("九键候选面板"));
                awaitGone(describedPrefix("候选 "));
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("输入模式恢复失败");
            restartIme();
        }
    }

    /** 迟到的松开不能提前通知；滑出和取消必须照常解除按下且不产生点击。 */
    private void checkReleaseLifecycle() throws ReflectiveOperationException {
        Context context = new ContextThemeWrapper(getTargetContext(), android.R.style.Theme_Material_Light);
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.KeyboardPressButton");
        View button = (View) type.getConstructor(Context.class).newInstance(context);
        button.layout(0, 0, 100, 100);
        int[] releases = {0}, clicks = {0};
        button.setOnClickListener(ignored -> clicks[0]++);
        type.getMethod("setPressReleaseListener", Runnable.class).invoke(button, (Runnable) () -> {
            if (button.isPressed()) throw new AssertionError("仍按下时收到松开通知");
            releases[0]++;
        });
        button.setPressed(true);
        dispatch(button, MotionEvent.ACTION_DOWN, 10);
        button.setPressed(false);
        if (!button.isPressed() || releases[0] != 0)
            throw new AssertionError("连按期间迟到的松开必须继续忽略");
        dispatch(button, MotionEvent.ACTION_CANCEL, 10);
        button.setPressed(false);
        if (button.isPressed() || releases[0] != 1 || clicks[0] != 0)
            throw new AssertionError("取消必须只通知一次且不点击");
        button.setPressed(true);
        dispatch(button, MotionEvent.ACTION_DOWN, 10);
        dispatch(button, MotionEvent.ACTION_MOVE, -100);
        dispatch(button, MotionEvent.ACTION_UP, -100);
        if (button.isPressed() || releases[0] != 2 || clicks[0] != 0)
            throw new AssertionError("滑出必须清除按下且不点击");
    }

    private static void dispatch(View button, int action, float x) {
        long now = SystemClock.uptimeMillis();
        MotionEvent event = MotionEvent.obtain(now, now, action, x, 10, 0);
        try { button.dispatchTouchEvent(event); }
        finally { event.recycle(); }
    }

    private void compose(boolean nineKey) throws Exception {
        if (nineKey) { tap(key("MNO")); tap(key("GHI")); }
        else { tap(described("字母 N")); tap(described("字母 I")); }
        await(candidate(1));
    }

    /** 不能用无障碍点击替代：它不进入 View 的按下、点击、延迟松开队列。 */
    private void touch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = awaitStableBounds(match);
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN,
            bounds.centerX(), bounds.centerY(), 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 100, MotionEvent.ACTION_UP,
            bounds.centerX(), bounds.centerY(), 0);
        down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(down, true)) throw new AssertionError("触摸按下未送达");
            SystemClock.sleep(100);
            if (!automation.injectInputEvent(up, true)) throw new AssertionError("触摸松开未送达");
        } finally { down.recycle(); up.recycle(); }
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            dropStaleNodes();
            boolean found = false;
            for (AccessibilityWindowInfo window : automation.getWindows())
                if (find(window.getRoot(), match) != null) { found = true; break; }
            if (!found) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("触摸选词后，候选界面仍未刷新");
    }

    private static String candidateText(AccessibilityNodeInfo node) {
        String description = node.getContentDescription().toString();
        String text = description.substring(description.indexOf('：') + 1);
        int annotation = text.indexOf('；');
        return annotation < 0 ? text : text.substring(0, annotation);
    }

    private Predicate<AccessibilityNodeInfo> completeCount() {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || node.getContentDescription() == null) return false;
            String value = node.getContentDescription().toString();
            if (!value.startsWith("完整候选列表；") || !value.endsWith(" 个候选")) return false;
            int start = value.lastIndexOf('；') + 1;
            try { return Integer.parseInt(value.substring(start, value.length() - 4)) > 9; }
            catch (NumberFormatException ignored) { return false; }
        };
    }

    private Predicate<AccessibilityNodeInfo> candidate(int index) {
        return describedPrefix("候选 " + index + "：").and(AccessibilityNodeInfo::isClickable);
    }
}
