package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import app.msime.android.core.DiagnosticConsentSmoke;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

/** 专用 AVD 的同意文件和真实系统退出夹具；调用方隔离产品网络并清理合成状态。 */
public final class DiagnosticConsentDeviceSmoke extends Instrumentation {
    private boolean revoke;
    private boolean systemCrash;
    private boolean authority;
    private String expectedRevision = "";
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        revoke = arguments != null && "revoke".equals(arguments.getString("phase"));
        systemCrash = arguments != null && "system-crash".equals(arguments.getString("phase"));
        // telemetry-authority.py 手动运行的只读授权夹具；不在这里分派时，它会落进下面的默认分支而误报通过。
        authority = arguments != null && "authority".equals(arguments.getString("phase"));
        if (authority) expectedRevision = arguments.getString("expected-revision", "");
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        int code = Activity.RESULT_CANCELED;
        Path root = null;
        try {
            if (systemCrash) {
                Class<?> telemetry = getTargetContext().getClassLoader().loadClass("app.msime.android.core.Telemetry");
                telemetry.getMethod("start", android.content.Context.class).invoke(null, getTargetContext());
                java.lang.reflect.Field enabled = telemetry.getDeclaredField("enabled");
                enabled.setAccessible(true);
                long deadline = android.os.SystemClock.uptimeMillis() + 15000;
                while (!enabled.getBoolean(null) && android.os.SystemClock.uptimeMillis() < deadline)
                    android.os.SystemClock.sleep(50);
                if (!enabled.getBoolean(null)) throw new AssertionError("capture preparation");
                synchronized (telemetry) {
                    // 仅夹具解除现场采集，保留真实系统摘要；其它进程不能抢先吞掉现场文件，让测试误把现场入队当成系统补录。
                    enabled.setBoolean(null, false);
                    try {
                        java.lang.reflect.Field active = telemetry.getDeclaredField("captureActive");
                        active.setAccessible(true);
                        active.setBoolean(null, false);
                        java.lang.reflect.Field watch = telemetry.getDeclaredField("consentWatch");
                        watch.setAccessible(true);
                        Object task = watch.get(null);
                        if (task instanceof java.util.concurrent.Future<?>) ((java.util.concurrent.Future<?>) task).cancel(false);
                    } catch (NoSuchFieldException legacy) { /* 旧版对照没有观察器。 */ }
                    Class<?> nativeClient = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
                    nativeClient.getMethod("telemetryCrashDisarm").invoke(null);
                }
                new android.os.Handler(android.os.Looper.getMainLooper()).post(() -> {
                    throw new IllegalStateException("synthetic system exit");
                });
                return;
            } else if (revoke) {
                Class<?> telemetry = getTargetContext().getClassLoader().loadClass("app.msime.android.core.Telemetry");
                telemetry.getMethod("setEnabled", android.content.Context.class, boolean.class)
                    .invoke(null, getTargetContext(), false);
                // 产品把核对交给串行的观察线程；夹具结束后进程会被收回，先等它执行完。
                java.lang.reflect.Field observer = telemetry.getDeclaredField("OBSERVER");
                observer.setAccessible(true);
                ((java.util.concurrent.ExecutorService) observer.get(null)).submit(() -> { })
                    .get(30, java.util.concurrent.TimeUnit.SECONDS);
            } else if (authority) {
                root = Files.createTempDirectory(getTargetContext().getFilesDir().toPath(), "synthetic-authority-");
                ReportingAuthorityDeviceChecks.run(root, getTargetContext().getClassLoader(), expectedRevision);
            } else {
                root = Files.createTempDirectory(getTargetContext().getFilesDir().toPath(), "synthetic-consent-");
                DiagnosticConsentSmoke.run(root);
            }
            code = Activity.RESULT_OK;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: diagnostic consent\n");
        } catch (Exception | AssertionError failure) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: diagnostic consent " + failure + "\n");
        } finally {
            if (root != null) {
                try (Stream<Path> paths = Files.walk(root)) {
                    for (Path path : (Iterable<Path>) paths.sorted(java.util.Comparator.reverseOrder())::iterator)
                        Files.deleteIfExists(path);
                } catch (Exception failure) { result.putString("cleanup", "synthetic consent cleanup failed"); }
            }
        }
        finish(code, result);
    }
}
