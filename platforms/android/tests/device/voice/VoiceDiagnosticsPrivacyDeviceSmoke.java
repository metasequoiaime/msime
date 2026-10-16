package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.os.ResultReceiver;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;

/** 隔离服务对象验证启动与接收两端的隐私门禁，不读取或改写用户设置和日志。 */
public final class VoiceDiagnosticsPrivacyDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Object service = null;
        Object gate = null;
        Object log = null;
        Path root = null;
        try {
            ClassLoader loader = getTargetContext().getClassLoader();
            Class<?> serviceType = loader.loadClass("app.msime.android.MSIMEInputService");
            Object[] created = new Object[1];
            runOnMainSync(() -> {
                try { created[0] = serviceType.getConstructor().newInstance(); }
                catch (ReflectiveOperationException error) { throw new AssertionError(error); }
            });
            service = created[0];
            root = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "voice-privacy-");
            java.io.File filesRoot = root.toFile();
            java.util.concurrent.atomic.AtomicBoolean filesUnavailable = new java.util.concurrent.atomic.AtomicBoolean();
            android.content.Context context = new android.content.ContextWrapper(getTargetContext()) {
                @Override public java.io.File getFilesDir() { return filesUnavailable.get() ? null : filesRoot; }
            };
            Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext",
                android.content.Context.class);
            attach.setAccessible(true);
            attach.invoke(service, context);
            Class<?> settings = loader.loadClass("app.msime.android.AndroidLocalSettings");
            Path settingsFile = (Path) settings.getMethod("file", android.content.Context.class)
                .invoke(null, context);
            Method update = settings.getMethod("update", Path.class, Map.class);
            Object enabled = update.invoke(null, settingsFile, Map.of("platform.android.developer.input_log", true));
            field(serviceType, "localSettings").set(service, enabled);
            field(serviceType, "allowLearning").setBoolean(service, true);
            field(serviceType, "editorInputType").setInt(service, 1);
            Class<?> gateType = loader.loadClass("app.msime.android.ImePrivacyGate");
            var gateConstructor = gateType.getDeclaredConstructor(serviceType);
            gateConstructor.setAccessible(true);
            gate = gateConstructor.newInstance(service);
            field(serviceType, "imePrivacyGate").set(service, gate);
            Class<?> overlayType = loader.loadClass("app.msime.android.ImeDebugOverlay");
            var overlayConstructor = overlayType.getDeclaredConstructor(serviceType);
            overlayConstructor.setAccessible(true);
            Object overlay = overlayConstructor.newInstance(service);
            field(serviceType, "imeDebugOverlay").set(service, overlay);
            Class<?> logType = loader.loadClass("app.msime.android.ImeDebugOverlay$EventLog");
            var logConstructor = logType.getDeclaredConstructor(java.io.File.class);
            logConstructor.setAccessible(true);
            log = logConstructor.newInstance(root.toFile());
            field(overlayType, "eventLog").set(overlay, log);
            Method receiverMethod = method(serviceType, "voiceDiagnosticsReceiver");
            Class<?> kindType = loader.loadClass("app.msime.android.DiagnosticEventKind");
            int code = ((Enum<?>) kindType.getMethod("fromWire", String.class)
                .invoke(null, "voice_failure_permission")).ordinal();
            ResultReceiver receiver = (ResultReceiver) receiverMethod.invoke(service);
            check(receiver != null);
            receiver.send(code, null);
            waitForIdleSync();
            // 普通按键与越界整数不能从语音窗口进入事件文件。
            receiver.send(0, null);
            receiver.send(Integer.MAX_VALUE, null);
            waitForIdleSync();
            for (String privacy : new String[] {"incognitoEnabled", "allowLearning", "editorInputType"}) {
                Field state = field(serviceType, privacy);
                Object previous = state.get(service);
                state.set(service, privacy.equals("incognitoEnabled") ? true
                    : privacy.equals("allowLearning") ? false : 0x81);
                check(receiverMethod.invoke(service) == null);
                receiver.send(code, null);
                waitForIdleSync();
                state.set(service, previous);
            }
            filesUnavailable.set(true);
            check(receiverMethod.invoke(service) == null);
            receiver.send(code, null);
            waitForIdleSync();
            filesUnavailable.set(false);
            // 保持键盘内存中的旧快照，模拟识别窗口活跃时用户在另一进程修改设置。
            update.invoke(null, settingsFile, Map.of("platform.android.incognito", true));
            check(receiverMethod.invoke(service) == null);
            receiver.send(code, null);
            waitForIdleSync();
            update.invoke(null, settingsFile, Map.of("platform.android.incognito", false,
                "platform.android.developer.input_log", false));
            check(receiverMethod.invoke(service) == null);
            receiver.send(code, null);
            waitForIdleSync();
            method(logType, "shutdown").invoke(log);
            check(((ExecutorService) field(logType, "worker").get(log)).awaitTermination(5, TimeUnit.SECONDS));
            String rows = new String(Files.readAllBytes(root.resolve("input-events.jsonl")),
                java.nio.charset.StandardCharsets.UTF_8);
            check(rows.split("\n").length == 1 && rows.contains("voice_failure_permission"));
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: voice diagnostics launch and receive privacy gates\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: voice diagnostics privacy ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            try {
                if (log != null) method(log.getClass(), "shutdown").invoke(log);
                if (gate != null) method(gate.getClass(), "release").invoke(gate);
                if (root != null) {
                    try (var paths = Files.walk(root)) {
                        var ordered = paths.sorted(java.util.Comparator.reverseOrder()).iterator();
                        while (ordered.hasNext()) Files.deleteIfExists(ordered.next());
                    }
                }
            } catch (Exception ignored) { /* 只清理本次测试创建的合成目录。 */ }
            if (service != null) {
                for (Field field : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(field.getType())) continue;
                    try {
                        field.setAccessible(true);
                        ((ExecutorService) field.get(service)).shutdownNow();
                    } catch (ReflectiveOperationException ignored) { /* 隔离服务没有启动输入会话。 */ }
                }
            }
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Method method(Class<?> type, String name) throws Exception {
        Method method = type.getDeclaredMethod(name);
        method.setAccessible(true);
        return method;
    }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("voice diagnostic privacy contract");
    }
}
