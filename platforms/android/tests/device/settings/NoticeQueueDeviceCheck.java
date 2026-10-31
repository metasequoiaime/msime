package app.msime.android.test;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.graphics.Rect;
import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.TimeUnit;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 延迟真实公告 JNI 的文件锁，验证设置内容不在公告后排队。 */
final class NoticeQueueDeviceCheck {
    private final DeviceSmoke test;
    private Activity activity;

    NoticeQueueDeviceCheck(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        AccessibilityServiceInfo info = test.automation.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
        test.automation.setServiceInfo(info);
        File directory = new File(test.getTargetContext().getFilesDir(), "notices");
        Files.createDirectories(directory.toPath());
        File cache = new File(directory, "notices.json");
        File lock = new File(directory, "notices.lock");
        byte[] original = null;
        boolean captured = false;
        try {
            JSONObject notice = new JSONObject().put("id", "synthetic-queue")
                .put("title", "合成公告队列验收").put("body", "仅用于设备回归")
                .put("published_at", "2026-10-22T00:00:00Z");
            JSONObject fixture = new JSONObject().put("feed", "app/android")
                .put("items", new JSONArray().put(notice)).put("dismissed", new JSONArray());
            test.stage = "hold notice fetch while opening native settings";
            try (Gate gate = new Gate(lock)) {
                gate.requireHeld();
                // 等首页已有请求释放公告锁，再备份和发布夹具，避免旧请求覆盖合成公告。
                original = cache.exists() ? Files.readAllBytes(cache.toPath()) : null;
                captured = true;
                fixture.put("attempted_at_unix_ms", System.currentTimeMillis());
                test.publish(cache, fixture.toString().getBytes(StandardCharsets.UTF_8));
                activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
                    "app.msime.android.home.HomeActivity")
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
                awaitNative("noticesRaw");
                checkSettings("fetch");
                test.stage = "notice fetch still pending after settings navigation";
                awaitNative("noticesRaw");
            }

            test.stage = "validate synthetic notice cache through shared JNI";
            awaitNativeIdle();
            test.waitForIdleSync();
            Class<?> bridge = test.getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
            String request = new JSONObject().put("directory", directory.getAbsolutePath())
                .put("platform", "android").put("channel", "app").toString();
            String response = (String) bridge.getMethod("notices", String.class).invoke(null, request);
            JSONObject value = new JSONObject(response).optJSONObject("value");
            JSONArray notices = value == null ? null : value.optJSONArray("items");
            if (notices == null || notices.length() != 1
                    || !"synthetic-queue".equals(notices.getJSONObject(0).optString("id")))
                throw new AssertionError("Synthetic notice cache was not accepted by shared JNI");
            test.stage = "notice result and dismissal remain functional";
            test.tap(described("返回"));
            homeTop();
            reveal("合成公告队列验收");
            try (Gate gate = new Gate(lock)) {
                gate.requireHeld();
                test.tap(text("知道了"));
                awaitNative("noticeDismissRaw");
                checkSettings("dismiss");
                test.stage = "notice dismissal still pending after settings navigation";
                awaitNative("noticeDismissRaw");
            }
            test.stage = "notice dismissal persisted after delayed completion";
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (!new JSONObject(new String(Files.readAllBytes(cache.toPath()), StandardCharsets.UTF_8)).getJSONArray("dismissed")
                    .toString().contains("synthetic-queue")) {
                if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Notice dismissal did not persist");
                SystemClock.sleep(50);
            }
        } finally {
            // 等锁内调用结束再恢复缓存；不能让迟到写入覆盖原始夹具。
            awaitNativeIdle();
            if (activity != null) {
                test.runOnMainSync(activity::finish);
                test.waitForIdleSync();
            }
            if (captured) {
                if (original == null) Files.deleteIfExists(cache.toPath());
                else test.publish(cache, original);
            }
        }
    }

    private void checkSettings(String operation) throws Exception {
        test.stage = operation + ": skin previews visible while notice JNI is blocked";
        homeTop();
        reveal("皮肤");
        test.tap(text("皮肤"));
        test.await(node -> host(node) && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("皮肤 "));
        test.tap(described("返回"));
        test.stage = operation + ": voice settings visible while notice JNI is blocked";
        reveal("语音输入");
        test.tap(text("语音输入"));
        test.await(text("识别语言"));
        test.await(text("离线识别"));

        test.stage = operation + ": background and resume retain actual settings content";
        test.shellDiscard("input keyevent 3");
        // `input keyevent 3` 注入后就返回，桌面由系统稍后拉起；先等设置窗口真正离开屏幕，否则 `am start` 会先落到仍在前台的任务上，迟到的 Home 再把它连同重建出的 Activity 一起推到后台。
        awaitBackground();
        test.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        test.await(text("识别语言"));

        test.stage = operation + ": activity recreation loads voice content while notice JNI is blocked";
        Instrumentation.ActivityMonitor monitor = test.addMonitor(activity.getClass().getName(), null, false);
        try {
            test.runOnMainSync(activity::recreate);
            Activity replacement = test.waitForMonitorWithTimeout(monitor, 15000);
            if (replacement == null) throw new AssertionError("Native settings activity was not recreated");
            activity = replacement;
        } finally {
            test.removeMonitor(monitor);
        }
        test.waitForIdleSync();
        test.dropStaleNodes();
        test.await(text("识别语言"));
    }

    private static Predicate<AccessibilityNodeInfo> id(String value) {
        return node -> host(node) && ("app.msime.android:id/" + value).equals(node.getViewIdResourceName());
    }

    private void awaitBackground() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            test.dropStaleNodes();
            if (!hostApplicationShown()) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Home key did not move native settings to the background");
    }

    private boolean hostApplicationShown() {
        for (AccessibilityWindowInfo window : test.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_APPLICATION) continue;
            AccessibilityNodeInfo root = window.getRoot();
            if (root != null && host(root)) return true;
        }
        return false;
    }

    private void homeTop() {
        for (int attempt = 0; attempt < 8; attempt++) {
            if (!test.await(id("keyboard_scroll")).performAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD)) break;
            SystemClock.sleep(200);
        }
    }

    private void reveal(String value) {
        Rect tabs = new Rect();
        test.await(id("tab_settings")).getBoundsInScreen(tabs);
        Predicate<AccessibilityNodeInfo> match = text(value).and(node -> {
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            return !bounds.isEmpty() && bounds.top >= 0 && bounds.bottom <= tabs.top;
        });
        for (int attempt = 0; attempt < 8; attempt++) {
            test.dropStaleNodes();
            for (var window : test.automation.getWindows()) {
                if (test.find(window.getRoot(), match) != null) return;
            }
            test.await(id("keyboard_scroll")).performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(200);
        }
        test.await(match);
    }

    private static boolean host(AccessibilityNodeInfo node) {
        return "app.msime.android".contentEquals(node.getPackageName() == null ? "" : node.getPackageName());
    }

    private static Predicate<AccessibilityNodeInfo> text(String value) {
        return node -> host(node) && value.contentEquals(node.getText() == null ? "" : node.getText());
    }

    private static Predicate<AccessibilityNodeInfo> described(String value) {
        return node -> host(node) && value.contentEquals(node.getContentDescription() == null ? "" : node.getContentDescription());
    }

    private void awaitNative(String method) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            if (nativePending(method)) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Expected notice JNI call did not reach the synthetic lock");
    }

    private static boolean nativePending(String method) {
        for (StackTraceElement[] trace : Thread.getAllStackTraces().values()) {
            for (StackTraceElement frame : trace) {
                if (frame.getClassName().equals("app.msime.android.NativeClient")
                        && frame.getMethodName().equals(method)) return true;
            }
        }
        return false;
    }

    private void awaitNativeIdle() {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (nativePending("noticesRaw") || nativePending("noticeDismissRaw")) {
            if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Notice JNI did not finish after releasing the lock");
            SystemClock.sleep(50);
        }
    }

    /**
     * flock 绑定打开文件描述；shell 保留同一描述符到标准输入关闭。
     * Android shell 的高位描述符带 CLOEXEC，用标准输出持锁，另存握手输出。
     */
    private static final class Gate implements AutoCloseable {
        private final Process process;

        Gate(File lock) throws Exception {
            process = new ProcessBuilder("/system/bin/sh", "-c",
                "exec 3>&1; exec 1>\"$1\"; toybox flock -x 1 || exit 1; echo locked >&3; read release",
                "msime-notice-gate", lock.getAbsolutePath()).redirectErrorStream(true).start();
            try {
                BufferedReader output = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8));
                long deadline = SystemClock.uptimeMillis() + 45000;
                while (!output.ready()) {
                    if (!process.isAlive() || SystemClock.uptimeMillis() >= deadline)
                        throw new AssertionError("Synthetic notice lock did not start");
                    SystemClock.sleep(20);
                }
                if (!"locked".equals(output.readLine())) throw new AssertionError("Synthetic notice lock failed");
            } catch (Exception | AssertionError error) {
                close();
                throw error;
            }
        }

        void requireHeld() {
            if (!process.isAlive()) throw new AssertionError("Synthetic notice lock stopped early");
        }

        @Override public void close() {
            try {
                process.getOutputStream().close();
                if (!process.waitFor(5, TimeUnit.SECONDS)) {
                    process.destroyForcibly();
                    throw new AssertionError("Synthetic notice lock did not stop");
                }
            } catch (java.io.IOException error) {
                process.destroyForcibly();
                throw new IllegalStateException("Synthetic notice lock pipe failed", error);
            } catch (InterruptedException error) {
                process.destroyForcibly();
                Thread.currentThread().interrupt();
                throw new IllegalStateException("Synthetic notice lock wait interrupted", error);
            }
        }
    }
}
