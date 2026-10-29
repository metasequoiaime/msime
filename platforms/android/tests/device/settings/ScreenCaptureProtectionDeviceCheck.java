package app.msime.android.test;

import android.app.Activity;
import android.app.Dialog;
import android.content.Intent;
import android.view.Window;
import android.view.WindowManager;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 「防止截屏」（#10098）落到真实窗口上：本地设置开着时，设置应用的 Activity 窗口和经 ScreenCaptureProtection 创建的对话框窗口都带 FLAG_SECURE，关上后重新打开的窗口不带。JVM 冒烟只覆盖切换规则和标志运算，check-host.sh 只看源码，这里读的是系统真正拿到的窗口属性。结束时把两项本地设置恢复成运行前的样子。
 */
final class ScreenCaptureProtectionDeviceCheck {
    private static final String ENABLED = "platform.android.screen_capture_protection";
    private static final String ENABLED_BEFORE = "platform.android.screen_capture_protection_enabled_before";

    private final DeviceSmoke test;
    private Class<?> settings;
    private Class<?> protection;

    ScreenCaptureProtectionDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        ClassLoader loader = test.getTargetContext().getClassLoader();
        settings = loader.loadClass("app.msime.android.AndroidLocalSettings");
        protection = loader.loadClass("app.msime.android.ScreenCaptureProtection");
        Object original = settings.getMethod("load", android.content.Context.class).invoke(null, test.getTargetContext());
        Map<String, Object> restore = new LinkedHashMap<>(2);
        // 运行前没写过的项恢复成「删掉」（null），回到默认值，而不是写下一个显式的默认值。
        for (String key : new String[] {ENABLED, ENABLED_BEFORE}) {
            boolean present = (Boolean) original.getClass().getMethod("has", String.class).invoke(original, key);
            restore.put(key, present ? original.getClass().getMethod("value", String.class).invoke(original, key) : null);
        }
        try {
            test.stage = "screen capture protection on: settings activity window is secure";
            write(true);
            Activity activity = launch();
            try {
                expectSecure(activity.getWindow(), true, "HomeActivity window");
                test.stage = "screen capture protection on: protected dialog window is secure";
                Dialog[] dialog = new Dialog[1];
                onMain(() -> {
                    dialog[0] = (Dialog) protection.getMethod("protect", Dialog.class).invoke(null, new Dialog(activity));
                    dialog[0].show();
                });
                try {
                    expectSecure(dialog[0].getWindow(), true, "protected dialog window");
                } finally {
                    test.runOnMainSync(dialog[0]::dismiss);
                }
            } finally {
                finish(activity);
            }

            test.stage = "screen capture protection off: reopened settings activity window is not secure";
            write(false);
            Activity reopened = launch();
            try {
                expectSecure(reopened.getWindow(), false, "reopened HomeActivity window");
            } finally {
                finish(reopened);
            }
        } finally {
            settings.getMethod("update", android.content.Context.class, Map.class).invoke(null, test.getTargetContext(), restore);
        }
    }

    private void write(boolean enabled) throws Exception {
        Map<String, Object> edits = new LinkedHashMap<>(2);
        edits.put(ENABLED, enabled);
        if (enabled) edits.put(ENABLED_BEFORE, true);
        settings.getMethod("update", android.content.Context.class, Map.class).invoke(null, test.getTargetContext(), edits);
    }

    private Activity launch() {
        Activity activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        test.waitForIdleSync();
        return activity;
    }

    private void finish(Activity activity) {
        test.runOnMainSync(activity::finish);
        test.waitForIdleSync();
    }

    private void expectSecure(Window window, boolean secure, String name) {
        boolean[] actual = new boolean[1];
        test.runOnMainSync(() -> actual[0] = window != null
            && (window.getAttributes().flags & WindowManager.LayoutParams.FLAG_SECURE) != 0);
        if (actual[0] != secure) throw new AssertionError(name + (secure ? " is missing FLAG_SECURE" : " still has FLAG_SECURE"));
    }

    private interface MainAction { void run() throws Exception; }

    // 主线程上的反射调用失败回传测试线程，不在主线程上抛出让被测进程崩溃。
    private void onMain(MainAction action) {
        Throwable[] failure = new Throwable[1];
        test.runOnMainSync(() -> {
            try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; }
        });
        if (failure[0] != null) throw new AssertionError("Screen capture protection setup failed: " + failure[0]);
    }
}
