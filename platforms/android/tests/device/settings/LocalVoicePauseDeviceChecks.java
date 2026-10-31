package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.SeekBar;
import java.util.LinkedHashMap;
import java.util.Map;

/** 经真实滑块的无障碍调整回调验证本地语音设置，不打开麦克风；结束时恢复原值和缺键状态。 */
final class LocalVoicePauseDeviceChecks {
    private static final String PAUSE = "platform.android.voice_pause_millis";
    private static final String SENSITIVITY = "platform.android.voice_vad_sensitivity";
    private final DeviceSmoke test;
    private Class<?> settings;
    private Activity activity;

    LocalVoicePauseDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        settings = test.getTargetContext().getClassLoader().loadClass("app.msime.android.AndroidLocalSettings");
        Object original = load();
        Map<String, Object> restore = new LinkedHashMap<>();
        Map<String, Object> defaults = new LinkedHashMap<>();
        for (String key : new String[] {PAUSE, SENSITIVITY}) {
            boolean present = (Boolean) original.getClass().getMethod("has", String.class).invoke(original, key);
            restore.put(key, present ? original.getClass().getMethod("value", String.class).invoke(original, key) : null);
            defaults.put(key, null);
        }
        try {
            update(defaults);
            test.stage = "local voice default sliders";
            launch();
            expectSlider("本地停顿时间", 1500);
            expectSlider("本地识别灵敏度", 50);
            test.stage = "local voice accessibility commits";
            adjust("本地停顿时间", 20);
            awaitValue(PAUSE, 2500);
            adjust("本地识别灵敏度", 60);
            awaitValue(SENSITIVITY, 70);
            test.stage = "local voice settings after page recreation";
            finish();
            launch();
            expectSlider("本地停顿时间", 2500);
            expectSlider("本地识别灵敏度", 70);
        } finally {
            try { finish(); } finally { update(restore); }
        }
    }

    private Object load() throws Exception {
        return settings.getMethod("load", Context.class).invoke(null, test.getTargetContext());
    }

    private void update(Map<String, Object> edits) throws Exception {
        settings.getMethod("update", Context.class, Map.class).invoke(null, test.getTargetContext(), edits);
    }

    private void awaitValue(String key, int expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            Object snapshot = load();
            if (((Integer) snapshot.getClass().getMethod("integer", String.class).invoke(snapshot, key)) == expected) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Local voice slider did not persist its value");
    }

    private void launch() throws java.util.concurrent.TimeoutException {
        activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
            .putExtra("msime_open_page", "VOICE"));
        AccessibilityNodeInfo ready = test.await(test.key("临时说明（首次运行）").or(test.key("识别语言")));
        if (DeviceSmoke.equalsText("临时说明（首次运行）", ready.getText())) test.tap(test.key("知道了"));
        test.await(node -> "识别语言".contentEquals(node.getText() == null ? "" : node.getText()));
        test.waitForIdleSync();
    }

    private void finish() {
        if (activity != null) {
            test.runOnMainSync(activity::finish);
            activity = null;
            test.waitForIdleSync();
        }
    }

    private SeekBar slider(String title) {
        return find(activity.getWindow().getDecorView(), title);
    }

    private static SeekBar find(View view, String title) {
        if (view instanceof SeekBar bar && bar.getContentDescription() != null
                && title.contentEquals(bar.getContentDescription())) return bar;
        if (view instanceof ViewGroup group) {
            for (int i = 0; i < group.getChildCount(); ++i) {
                SeekBar found = find(group.getChildAt(i), title);
                if (found != null) return found;
            }
        }
        return null;
    }

    private void expectSlider(String title, int expected) {
        onMain(() -> {
            SeekBar bar = slider(title);
            if (bar == null) throw new AssertionError("Missing local voice slider");
            try {
                int actual = (Integer) bar.getClass().getMethod("value").invoke(bar);
                if (actual != expected) throw new AssertionError("Unexpected local voice slider value");
            } catch (ReflectiveOperationException error) {
                throw new AssertionError("Local voice slider value unavailable", error);
            }
        });
    }

    private void adjust(String title, int slot) {
        onMain(() -> {
            SeekBar bar = slider(title);
            if (bar == null) throw new AssertionError("Missing local voice slider");
            Bundle args = new Bundle();
            args.putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, slot);
            if (!bar.performAccessibilityAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId(), args))
                throw new AssertionError("Local voice slider rejected accessibility adjustment");
        });
        test.waitForIdleSync();
    }

    private void onMain(Runnable action) {
        Throwable[] failure = new Throwable[1];
        test.runOnMainSync(() -> {
            try { action.run(); } catch (Throwable error) { failure[0] = error; }
        });
        if (failure[0] != null) throw new AssertionError("Local voice control assertion failed", failure[0]);
    }
}
