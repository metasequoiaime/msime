package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.os.SystemClock;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import java.lang.reflect.Method;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONObject;

/** 经真实设置页及共享存储验证四种旧状态、组合选择和工具栏独立开关；不启动录音。 */
final class VoiceTriggersDeviceCheck {
    private static final String SPACE = "platform.android.space_voice";
    private static final String TOOLBAR = "touch_voice_shortcut";
    private final DeviceSmoke test;
    private Class<?> local;
    private Class<?> store;
    private Activity activity;
    private Object page;

    VoiceTriggersDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        ClassLoader loader = test.getTargetContext().getClassLoader();
        local = loader.loadClass("app.msime.android.AndroidLocalSettings");
        store = loader.loadClass("app.msime.android.home.HostStore");
        Object before = local.getMethod("load", Context.class).invoke(null, test.getTargetContext());
        boolean hadSpace = (Boolean) before.getClass().getMethod("has", String.class).invoke(before, SPACE);
        Object spaceBefore = before.getClass().getMethod("value", String.class).invoke(before, SPACE);
        JSONObject sharedBefore = preferences();
        boolean hadToolbar = sharedBefore.has(TOOLBAR);
        Object toolbarBefore = sharedBefore.opt(TOOLBAR);
        try {
            for (boolean space : new boolean[] {false, true}) for (boolean toolbar : new boolean[] {false, true}) {
                test.stage = "voice trigger existing state " + space + "/" + toolbar;
                writeSpace(space);
                writeToolbar(toolbar);
                launch("VOICE", "app.msime.android.home.VoicePage");
                awaitLabel(space || toolbar ? "工具栏/长按空格" : "无");
                expect(space, toolbar);
                // 直接走设置回调及其工作线程，覆盖重复选择组合项和「无」的实际落盘。
                invoke("saveTrigger", new Class<?>[] {int.class}, 0);
                awaitState(true, toolbar);
                invoke("saveTrigger", new Class<?>[] {int.class}, 1);
                awaitState(false, false);
                close();
            }
            test.stage = "toolbar voice switch preserves long-press space";
            for (boolean space : new boolean[] {false, true}) {
                writeSpace(space);
                launch("KEYBOARD_OPTIONS", "app.msime.android.home.KeyboardOptionsPage");
                for (boolean toolbar : new boolean[] {true, false}) {
                    invoke("saveToolbar", new Class<?>[] {String.class, boolean.class}, TOOLBAR, toolbar);
                    awaitState(space, toolbar);
                }
                close();
            }
        } finally {
            close();
            Map<String, Object> restore = new LinkedHashMap<>();
            restore.put(SPACE, hadSpace ? spaceBefore : null);
            local.getMethod("update", Context.class, Map.class).invoke(null, test.getTargetContext(), restore);
            JSONObject snapshot = snapshot();
            JSONObject values = snapshot.getJSONObject("preferences");
            if (hadToolbar) values.put(TOOLBAR, toolbarBefore); else values.remove(TOOLBAR);
            save(snapshot);
        }
    }

    private void launch(String id, String className) throws Exception {
        Class<?> type = test.getTargetContext().getClassLoader().loadClass(className);
        activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK)
            .putExtra("msime_open_page", id));
        long deadline = SystemClock.uptimeMillis() + 10000;
        while (page == null && SystemClock.uptimeMillis() < deadline) {
            onMain(() -> {
                Object manager = activity.getClass().getMethod("getSupportFragmentManager").invoke(activity);
                for (Object fragment : (List<?>) manager.getClass().getMethod("getFragments").invoke(manager))
                    if (type.isInstance(fragment)) page = fragment;
            });
            if (page == null) SystemClock.sleep(100);
        }
        require(page != null, "原生导航创建目标设置页");
        test.waitForIdleSync();
    }

    private void invoke(String name, Class<?>[] types, Object... args) throws Exception {
        onMain(() -> {
            Method method = page.getClass().getDeclaredMethod(name, types);
            method.setAccessible(true);
            method.invoke(page, args);
        });
    }

    private void awaitLabel(String label) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        boolean[] found = {false};
        do {
            onMain(() -> found[0] = contains(activity.getWindow().getDecorView(), label));
            if (!found[0]) SystemClock.sleep(100);
        } while (!found[0] && SystemClock.uptimeMillis() < deadline);
        require(found[0], "启动方式显示预期选项");
    }

    private static boolean contains(View root, String label) {
        if (root instanceof TextView text && label.contentEquals(text.getText())) return true;
        if (root instanceof ViewGroup group) for (int index = 0; index < group.getChildCount(); index++)
            if (contains(group.getChildAt(index), label)) return true;
        return false;
    }

    private boolean space() throws Exception {
        Object state = local.getMethod("load", Context.class).invoke(null, test.getTargetContext());
        return (Boolean) state.getClass().getMethod("bool", String.class).invoke(state, SPACE);
    }

    private JSONObject snapshot() throws Exception {
        JSONObject value = (JSONObject) store.getMethod("loadPreferences", Context.class).invoke(null, test.getTargetContext());
        require(value != null, "共享偏好可读");
        return value;
    }

    private JSONObject preferences() throws Exception { return snapshot().getJSONObject("preferences"); }
    private void save(JSONObject snapshot) throws Exception {
        require(store.getMethod("savePreferences", Context.class, JSONObject.class)
            .invoke(null, test.getTargetContext(), snapshot) != null, "共享偏好保存成功");
    }
    private void writeToolbar(boolean value) throws Exception {
        JSONObject snapshot = snapshot();
        snapshot.getJSONObject("preferences").put(TOOLBAR, value);
        save(snapshot);
    }
    private void writeSpace(boolean value) throws Exception {
        local.getMethod("update", Context.class, Map.class).invoke(null, test.getTargetContext(), Map.of(SPACE, value));
    }
    private void expect(boolean space, boolean toolbar) throws Exception {
        require(space() == space && preferences().optBoolean(TOOLBAR, false) == toolbar, "两个入口按预期独立保存");
    }
    private void awaitState(boolean space, boolean toolbar) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        while ((space() != space || preferences().optBoolean(TOOLBAR, false) != toolbar)
                && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(100);
        expect(space, toolbar);
        test.waitForIdleSync();
    }
    private void close() {
        if (activity != null) { test.runOnMainSync(activity::finish); test.waitForIdleSync(); }
        activity = null;
        page = null;
    }
    private interface Action { void run() throws Exception; }
    private void onMain(Action action) throws Exception {
        Throwable[] failure = new Throwable[1];
        test.runOnMainSync(() -> { try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; } });
        if (failure[0] != null) throw new AssertionError("语音入口设置操作失败", failure[0]);
    }
    private static void require(boolean value, String message) { if (!value) throw new AssertionError(message); }
}
