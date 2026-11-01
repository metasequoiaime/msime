package app.msime.android.test;

import android.app.Activity;
import android.app.Application;
import android.app.Instrumentation;
import android.os.Bundle;
import android.os.SystemClock;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.json.JSONObject;

/** 真实 :ime 进程中的合成故障夹具，设置由外部测试进程修改；不代替产品执行同意刷新。 */
public final class TelemetryWatchDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    private static Object field(Class<?> owner, String name, Object fallback) throws Exception {
        try {
            Field field = owner.getDeclaredField(name);
            field.setAccessible(true);
            return field.get(null);
        } catch (NoSuchFieldException missing) { return fallback; }
    }

    /** 使用产品 JNI 保存真实偏好，不调用遥测刷新；代次必须由共享存储维护。 */
    private void saveConsent(boolean enabled) throws Exception {
        Class<?> nativeClient = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            getTargetContext().getFilesDir().toPath().resolve("runtime-options.json")), StandardCharsets.UTF_8));
        String preferences = options.getString("preferences_directory");
        JSONObject loaded = new JSONObject((String) nativeClient.getMethod("loadPreferences", String.class).invoke(null, preferences));
        if (!loaded.getBoolean("ok")) throw new AssertionError("load preferences");
        JSONObject snapshot = loaded.getJSONObject("value");
        snapshot.getJSONObject("preferences").put("usage_reporting", enabled);
        JSONObject saved = new JSONObject((String) nativeClient.getMethod("savePreferences", String.class, long.class, String.class)
            .invoke(null, preferences, snapshot.getLong("revision"), snapshot.toString()));
        if (!saved.getBoolean("ok")) throw new AssertionError("save preferences");
    }

    /** 夹具代表正在显示的键盘：产品只在键盘显示期间轮询授权，开始会话后按 onStartInputView 的顺序恢复观察。 */
    private void showKeyboard(Class<?> telemetry) throws Exception {
        telemetry.getMethod("resumeConsentWatch", android.content.Context.class).invoke(null, getTargetContext());
        // resumeConsentWatch 只把恢复排进串行的观察线程；首轮准备在 WORKER 上武装捕获，两者争同一把 Telemetry 锁。不等恢复执行完就写状态，可能写出「已武装、却还没在观察」的中间态，外部测试会把它当成首个就绪状态而判失败。等观察线程上一个排在恢复之后的空任务，报告的就是键盘已显示后的状态。
        ((java.util.concurrent.ExecutorService) field(telemetry, "OBSERVER", null)).submit(() -> { })
            .get(30, java.util.concurrent.TimeUnit.SECONDS);
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        int code = Activity.RESULT_CANCELED;
        Path root = getTargetContext().getFilesDir().toPath().resolve("synthetic-telemetry-watch");
        Class<?> telemetry = null;
        boolean ended = false;
        boolean fault = false;
        try {
            if (!Application.getProcessName().equals("app.msime.android:ime")) throw new AssertionError("requires :ime");
            Files.createDirectories(root);
            telemetry = getTargetContext().getClassLoader().loadClass("app.msime.android.core.Telemetry");
            telemetry.getMethod("beginInputSession", android.content.Context.class).invoke(null, getTargetContext());
            showKeyboard(telemetry);
            String completed = "";
            long deadline = SystemClock.uptimeMillis() + 180000;
            while (SystemClock.uptimeMillis() < deadline) {
                JSONObject status;
                synchronized (telemetry) {
                    Object watch = field(telemetry, "consentWatch", null);
                    status = new JSONObject().put("enabled", field(telemetry, "enabled", false))
                        .put("session_id", field(telemetry, "sessionId", ""))
                        .put("epoch", field(telemetry, "captureEpoch", ""))
                        .put("header", field(telemetry, "crashRecordHeader", ""))
                        .put("completed", completed)
                        .put("capture", String.valueOf(field(telemetry, "sessionCrashRecord", "")))
                        .put("watching", watch instanceof java.util.concurrent.Future<?>
                            && !((java.util.concurrent.Future<?>) watch).isDone())
                        .put("pid", android.os.Process.myPid());
                }
                Path incoming = root.resolve("status.tmp");
                Files.write(incoming, status.toString().getBytes(StandardCharsets.UTF_8));
                Files.move(incoming, root.resolve("status.json"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                Path commandPath = root.resolve("command");
                String command = Files.exists(commandPath)
                    ? new String(Files.readAllBytes(commandPath), StandardCharsets.US_ASCII).trim() : "";
                if (!command.isEmpty()) Files.deleteIfExists(commandPath);
                if (command.equals("cycle")) {
                    synchronized (telemetry) {
                        saveConsent(false);
                        saveConsent(true);
                    }
                }
                if (command.equals("stale-close")) {
                    telemetry.getMethod("setEnabled", android.content.Context.class, boolean.class)
                        .invoke(null, getTargetContext(), false);
                    // 产品把核对交给串行的观察线程；等它执行完再报告完成，检查的才是迟到关闭的结果。
                    ((java.util.concurrent.ExecutorService) field(telemetry, "OBSERVER", null)).submit(() -> { })
                        .get(30, java.util.concurrent.TimeUnit.SECONDS);
                }
                if (!command.isEmpty()) completed = command;
                if (command.equals("java")) {
                    fault = true;
                    new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> {
                        throw new IllegalStateException("synthetic consent watch fault");
                    });
                    return;
                }
                if (command.equals("native")) {
                    fault = true;
                    android.os.Process.sendSignal(android.os.Process.myPid(), 6);
                    throw new AssertionError("fatal signal returned");
                }
                if (command.equals("end")) {
                    telemetry.getMethod("endInputSession", android.content.Context.class).invoke(null, getTargetContext());
                    ended = true;
                }
                if (command.equals("begin") || command.equals("application")) {
                    telemetry.getMethod(command.equals("begin") ? "beginInputSession" : "start", android.content.Context.class)
                        .invoke(null, getTargetContext());
                    showKeyboard(telemetry);
                    ended = false;
                }
                if (command.equals("finish")) {
                    code = Activity.RESULT_OK;
                    result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: telemetry watch\n");
                    break;
                }
                SystemClock.sleep(100);
            }
        } catch (Exception | AssertionError failure) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: telemetry watch " + failure.getClass().getSimpleName() + "\n");
        } finally {
            if (!fault && !ended && telemetry != null) {
                try { telemetry.getMethod("endInputSession", android.content.Context.class).invoke(null, getTargetContext()); }
                catch (Exception ignored) { result.putString("cleanup", "session end failed"); }
            }
        }
        finish(code, result);
    }
}
