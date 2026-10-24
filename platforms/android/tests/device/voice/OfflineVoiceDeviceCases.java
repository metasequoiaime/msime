package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.app.PendingIntent;
import android.app.UiAutomation;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.MediaRecorder;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.speech.RecognizerIntent;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/** 实际产品进程中的启动取消边界与标准结果通道；合成结果不代表模型识别验收。 */
final class OfflineVoiceDeviceCases {
    private final Instrumentation instrumentation;
    private final String scenario;
    private volatile Activity activity;
    private volatile Throwable failure;
    private final java.util.Set<String> permissionNodes = new java.util.TreeSet<>();
    private final java.util.List<String> stages = new java.util.concurrent.CopyOnWriteArrayList<>();

    OfflineVoiceDeviceCases(Instrumentation instrumentation, String scenario) {
        this.instrumentation = instrumentation;
        this.scenario = scenario;
    }

    void onCreated(Activity created) {
        if (!created.getClass().getName().equals("app.msime.android.OfflineVoiceRecognitionActivity")) return;
        activity = created;
        if (scenario.equals("offline-asr") || scenario.equals("offline-no-model")) {
            try {
                var field = created.getClass().getDeclaredField("recognizer");
                field.setAccessible(true);
                Object recognizer = field.get(created);
                java.util.function.Consumer<Object> diagnostics = kind -> stages.add(kind.toString());
                recognizer.getClass().getMethod("diagnostics", java.util.function.Consumer.class)
                    .invoke(recognizer, diagnostics);
            } catch (ReflectiveOperationException error) {
                failure = error;
            }
        }
        if (!java.util.Set.of("offline-result", "offline-cancel", "offline-activity-result",
                "offline-activity-cancel", "offline-repeat").contains(scenario)) return;
        try {
            if (scenario.startsWith("offline-activity-")) {
                if (created.getCallingActivity() == null) throw new AssertionError("Caller identity missing");
                if (created.isFinishing()) throw new AssertionError("Standard Activity request rejected");
            }
            var deliver = created.getClass().getDeclaredMethod("returnResult", String.class);
            deliver.setAccessible(true);
            String text = scenario.equals("offline-result") || scenario.equals("offline-activity-result") || scenario.equals("offline-repeat")
                ? "synthetic external transcript" : null;
            deliver.invoke(created, text);
            deliver.invoke(created, "synthetic late transcript");
        } catch (ReflectiveOperationException error) {
            failure = error;
        }
    }

    void run() {
        Bundle result = new Bundle();
        try {
            if (scenario.equals("offline-prepare")) prepareLocalModel();
            else if (scenario.equals("offline-capture-cancel")) captureCancellation();
            else if (scenario.equals("offline-no-model")) noModelResult();
            else if (scenario.startsWith("offline-activity-")) activityResult();
            else if (scenario.equals("offline-repeat")) {
                for (int request = 0; request < 3; request++) {
                    activity = null;
                    failure = null;
                    externalResult();
                }
            } else externalResult();
            result.putString("stream", "Offline voice boundary passed: " + scenario);
            instrumentation.finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "Offline voice boundary failed: " + error.getClass().getSimpleName()
                + ": " + error.getMessage());
            instrumentation.finish(Activity.RESULT_CANCELED, result);
        }
    }

    /** 不同 UID 的测试应用必须通过 onActivityResult 接到单次成功或不带文字的取消。 */
    private void activityResult() throws Exception {
        Context context = instrumentation.getTargetContext();
        var caller = context.getPackageManager().getApplicationInfo("app.msime.android.test", 0);
        if (caller.uid == context.getApplicationInfo().uid) throw new AssertionError("Caller is not isolated");
        AtomicInteger reports = new AtomicInteger();
        AtomicInteger code = new AtomicInteger();
        AtomicReference<Intent> returned = new AtomicReference<>();
        String action = context.getPackageName() + ".OFFLINE_CALLER." + java.util.UUID.randomUUID();
        BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override public void onReceive(Context ignored, Intent intent) {
                code.set(getResultCode());
                returned.set(intent);
                reports.incrementAndGet();
            }
        };
        if (Build.VERSION.SDK_INT >= 33) context.registerReceiver(receiver, new IntentFilter(action), Context.RECEIVER_NOT_EXPORTED);
        else context.registerReceiver(receiver, new IntentFilter(action));
        int flags = PendingIntent.FLAG_CANCEL_CURRENT;
        if (Build.VERSION.SDK_INT >= 31) flags |= PendingIntent.FLAG_MUTABLE;
        PendingIntent report = PendingIntent.getBroadcast(context, 9159,
            new Intent(action).setPackage(context.getPackageName()), flags);
        Intent launch = new Intent().setClassName("app.msime.android.test",
            "app.msime.android.test.OfflineVoiceCallerActivity")
            .putExtra(OfflineVoiceCallerActivity.REPORT, report)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        try (StorageBoundary storage = new StorageBoundary(context)) {
            storage.prepare();
            instrumentation.runOnMainSync(() -> context.startActivity(launch));
            await(() -> activity != null, "External activity was not called by the fixture");
            await(() -> reports.get() > 0, "Standard Activity result did not reach the separate caller");
            if (failure != null) throw new AssertionError("Activity callback failed", failure);
            boolean success = scenario.equals("offline-activity-result");
            Intent observed = returned.get();
            if (reports.get() != 1 || observed.getIntExtra("caller_deliveries", 0) != 1)
                throw new AssertionError("Caller observed repeated Activity results");
            if (code.get() != (success ? Activity.RESULT_OK : Activity.RESULT_CANCELED))
                throw new AssertionError("Caller observed an incorrect Activity result code");
            if (success) {
                if (!java.util.List.of("synthetic external transcript").equals(
                        observed.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)))
                    throw new AssertionError("Caller observed an incorrect transcript");
            } else if (observed.hasExtra(RecognizerIntent.EXTRA_RESULTS)) {
                throw new AssertionError("Cancelled Activity result contains text");
            }
            if (!finished()) throw new AssertionError("Completed request is still visible");
            storage.verify();
        } finally {
            context.unregisterReceiver(receiver);
            report.cancel();
            if (activity != null) instrumentation.runOnMainSync(activity::finish);
            try (var input = instrumentation.getUiAutomation().executeShellCommand(
                    "am force-stop app.msime.android.test")) {
                try (var stream = new java.io.FileInputStream(input.getFileDescriptor())) {
                    byte[] discard = new byte[4096];
                    while (stream.read(discard) != -1) { }
                }
            }
        }
    }

    /** 用同一 UID 的前台窗口观察剪贴板，并保护已有的语音待插入值。 */
    private final class StorageBoundary implements java.io.Closeable {
        private static final String SENTINEL = "synthetic offline boundary sentinel";
        private final Context context;
        private final java.util.List<Activity> homes = new java.util.ArrayList<>();
        private Object store;
        private Object saved;
        private Class<?> storeType;
        private android.content.ClipData originalClipboard;
        private boolean clipboardSeeded;

        StorageBoundary(Context context) { this.context = context; }

        private void foreground() {
            homes.add(instrumentation.startActivitySync(new Intent().setClassName(context,
                "app.msime.android.home.HomeActivity")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_MULTIPLE_TASK)));
            instrumentation.waitForIdleSync();
            await(() -> {
                AtomicBoolean focus = new AtomicBoolean();
                instrumentation.runOnMainSync(() -> focus.set(homes.get(homes.size() - 1).hasWindowFocus()));
                return focus.get();
            }, "Storage observer did not receive foreground focus");
        }

        void prepare() throws Exception {
            foreground();
            storeType = context.getClassLoader().loadClass("app.msime.android.VoiceResultStore");
            store = storeType.getConstructor(java.nio.file.Path.class)
                .newInstance(context.getFilesDir().toPath().resolve("voice-handoff"));
            Object existing = storeType.getMethod("read", long.class).invoke(store, System.currentTimeMillis());
            if (existing != null) {
                // 中断的本用例可留下同一合成值；其它队列内容一律不覆盖。
                Object text = existing.getClass().getMethod("text").invoke(existing);
                if (!SENTINEL.equals(text)) throw new AssertionError("Dedicated device voice queue is not empty");
                saved = existing;
            } else {
                saved = storeType.getMethod("save", String.class, long.class)
                    .invoke(store, SENTINEL, System.currentTimeMillis());
            }
            instrumentation.runOnMainSync(() -> {
                var clipboard = context.getSystemService(android.content.ClipboardManager.class);
                originalClipboard = clipboard.getPrimaryClip();
                clipboard.setPrimaryClip(android.content.ClipData.newPlainText("synthetic boundary", SENTINEL));
                clipboardSeeded = true;
                assertClipboard(clipboard);
            });
        }

        void verify() throws Exception {
            foreground();
            Object observed = storeType.getMethod("read", long.class).invoke(store, System.currentTimeMillis());
            if (!saved.equals(observed)) throw new AssertionError("External voice changed the IME result queue");
            instrumentation.runOnMainSync(() -> assertClipboard(
                context.getSystemService(android.content.ClipboardManager.class)));
        }

        private void assertClipboard(android.content.ClipboardManager clipboard) {
            var clip = clipboard.getPrimaryClip();
            if (clip == null || clip.getItemCount() != 1 || !SENTINEL.contentEquals(clip.getItemAt(0).getText()))
                throw new AssertionError("External voice changed the system clipboard");
        }

        @Override public void close() throws java.io.IOException {
            Throwable failure = null;
            try {
                if (clipboardSeeded) {
                    foreground();
                    instrumentation.runOnMainSync(() -> {
                        var clipboard = context.getSystemService(android.content.ClipboardManager.class);
                        if (originalClipboard == null) clipboard.clearPrimaryClip();
                        else clipboard.setPrimaryClip(originalClipboard);
                    });
                }
            } catch (Exception | AssertionError error) {
                failure = error;
            }
            try {
                if (saved != null) {
                    java.util.UUID id = (java.util.UUID) saved.getClass().getMethod("id").invoke(saved);
                    Object consumed = storeType.getMethod("consume", java.util.UUID.class, long.class)
                        .invoke(store, id, System.currentTimeMillis());
                    if (!SENTINEL.equals(consumed)) throw new AssertionError("Synthetic queue cleanup failed");
                }
            } catch (Exception | AssertionError error) {
                if (failure == null) failure = error;
                else failure.addSuppressed(error);
            }
            for (Activity home : homes) {
                try {
                    instrumentation.runOnMainSync(home::finish);
                } catch (RuntimeException | AssertionError error) {
                    if (failure == null) failure = error;
                    else failure.addSuppressed(error);
                }
            }
            if (failure instanceof AssertionError error) throw error;
            if (failure != null) throw new java.io.IOException("Synthetic storage cleanup failed", failure);
        }
    }

    /** 专用模拟器只导入预先校验的合成验收资源，安装与配置保存仍走产品共享接口。 */
    private void prepareLocalModel() throws Exception {
        Context context = instrumentation.getTargetContext();
        Activity home = instrumentation.startActivitySync(new Intent().setClassName(context,
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        java.io.File options = new java.io.File(context.getFilesDir(), "runtime-options.json");
        try {
            await(options::isFile, "Native host did not prepare runtime options");
            Class<?> client = context.getClassLoader().loadClass("app.msime.android.NativeClient");
            Class<?> progress = context.getClassLoader().loadClass("app.msime.android.NativeClient$LocalModelProgress");
            java.io.File archive = new java.io.File(context.getCacheDir(), "synthetic-offline-model.tar.bz2");
            try {
                try (var descriptor = instrumentation.getUiAutomation().executeShellCommand(
                        "cat /data/local/tmp/msime-synthetic-offline-model.tar.bz2");
                        var input = new java.io.FileInputStream(descriptor.getFileDescriptor());
                        var output = new java.io.FileOutputStream(archive)) {
                    byte[] buffer = new byte[64 * 1024];
                    int count;
                    while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
                }
                String request = new org.json.JSONObject()
                    .put("root", new java.io.File(context.getFilesDir(), "voice-models").getAbsolutePath())
                    .put("id", "x-asr-zh-en-streaming")
                    .put("files", new org.json.JSONArray().put(archive.getAbsolutePath())).toString();
                Object installed = client.getMethod("voiceLocalModelInstall", String.class, progress)
                    .invoke(null, request, null);
                org.json.JSONObject model = new org.json.JSONObject((String) installed);
                if (!model.optBoolean("ok")) throw new AssertionError("Pinned model import failed: " + model.optString("error"));
                String directory = new org.json.JSONObject(new String(java.nio.file.Files.readAllBytes(options.toPath()),
                    java.nio.charset.StandardCharsets.UTF_8))
                    .getString("preferences_directory");
                org.json.JSONObject loaded = new org.json.JSONObject((String) client
                    .getMethod("loadPreferences", String.class).invoke(null, directory));
                if (!loaded.optBoolean("ok")) throw new AssertionError("Shared preferences unavailable");
                org.json.JSONObject snapshot = loaded.getJSONObject("value");
                org.json.JSONObject preferences = snapshot.getJSONObject("preferences");
                org.json.JSONObject voice = preferences.optJSONObject("voice_input");
                if (voice == null) preferences.put("voice_input", voice = new org.json.JSONObject());
                voice.put("asr_provider", "local").put("asr_model_path", model.getJSONObject("value").getString("path"));
                org.json.JSONObject saved = new org.json.JSONObject((String) client
                    .getMethod("savePreferences", String.class, long.class, String.class)
                    .invoke(null, directory, snapshot.getLong("revision"), snapshot.toString()));
                if (!saved.optBoolean("ok")) throw new AssertionError("Shared local model selection failed");
            } finally {
                java.nio.file.Files.deleteIfExists(archive.toPath());
            }
        } finally {
            instrumentation.runOnMainSync(home::finish);
        }
    }

    /** 只临时更改共享配置的模型选择，不删除已安装资源；恢复时保留其它设置的新改动。 */
    private void noModelResult() throws Exception {
        Context context = instrumentation.getTargetContext();
        Activity home = instrumentation.startActivitySync(new Intent().setClassName(context,
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        try {
            java.io.File options = new java.io.File(context.getFilesDir(), "runtime-options.json");
            await(options::isFile, "Native host did not prepare runtime options");
            String directory = new org.json.JSONObject(new String(java.nio.file.Files.readAllBytes(options.toPath()),
                java.nio.charset.StandardCharsets.UTF_8))
                .getString("preferences_directory");
            Class<?> client = context.getClassLoader().loadClass("app.msime.android.NativeClient");
            var load = client.getMethod("loadPreferences", String.class);
            var save = client.getMethod("savePreferences", String.class, long.class, String.class);
            org.json.JSONObject loaded = new org.json.JSONObject((String) load.invoke(null, directory));
            if (!loaded.optBoolean("ok")) throw new AssertionError("Shared preferences unavailable");
            org.json.JSONObject snapshot = loaded.getJSONObject("value");
            org.json.JSONObject voice = snapshot.getJSONObject("preferences").getJSONObject("voice_input");
            String originalProvider = voice.getString("asr_provider");
            String originalModel = voice.getString("asr_model_path");
            voice.put("asr_provider", "local").put("asr_model_path", "");
            org.json.JSONObject saved = new org.json.JSONObject((String) save.invoke(null, directory,
                snapshot.getLong("revision"), snapshot.toString()));
            if (!saved.optBoolean("ok")) throw new AssertionError("Synthetic model selection failed");
            try {
                externalResult();
            } finally {
                loaded = new org.json.JSONObject((String) load.invoke(null, directory));
                if (!loaded.optBoolean("ok")) throw new AssertionError("Cannot load preferences for restoration");
                snapshot = loaded.getJSONObject("value");
                voice = snapshot.getJSONObject("preferences").getJSONObject("voice_input");
                if (!"local".equals(voice.getString("asr_provider")) || !voice.getString("asr_model_path").isEmpty())
                    throw new AssertionError("Concurrent model selection must not be overwritten");
                voice.put("asr_provider", originalProvider).put("asr_model_path", originalModel);
                saved = new org.json.JSONObject((String) save.invoke(null, directory,
                    snapshot.getLong("revision"), snapshot.toString()));
                if (!saved.optBoolean("ok")) throw new AssertionError("Model selection restoration failed");
                loaded = new org.json.JSONObject((String) load.invoke(null, directory));
                if (!loaded.optBoolean("ok")) throw new AssertionError("Cannot verify restored preferences");
                voice = loaded.getJSONObject("value").getJSONObject("preferences").getJSONObject("voice_input");
                if (!originalProvider.equals(voice.getString("asr_provider"))
                        || !originalModel.equals(voice.getString("asr_model_path")))
                    throw new AssertionError("Original model selection was not restored");
            }
        } finally {
            instrumentation.runOnMainSync(home::finish);
        }
    }

    private void externalResult() throws Exception {
        Context context = instrumentation.getTargetContext();
        boolean reconstruction = scenario.equals("offline-recreate") || scenario.equals("offline-rotation");
        if ((scenario.equals("offline-permission-home") || scenario.equals("offline-permission-deny") || reconstruction) && context.checkSelfPermission(
                android.Manifest.permission.RECORD_AUDIO) == android.content.pm.PackageManager.PERMISSION_GRANTED) {
            throw new AssertionError("Revoke RECORD_AUDIO before testing a permission-stage request");
        }
        AtomicInteger deliveries = new AtomicInteger();
        AtomicReference<Intent> returned = new AtomicReference<>();
        AtomicInteger code = new AtomicInteger();
        String action = context.getPackageName() + ".OFFLINE_SYNTHETIC_RESULT." + java.util.UUID.randomUUID();
        BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override public void onReceive(Context ignored, Intent intent) {
                code.set(getResultCode());
                returned.set(intent);
                deliveries.incrementAndGet();
            }
        };
        if (Build.VERSION.SDK_INT >= 33) context.registerReceiver(receiver, new IntentFilter(action), Context.RECEIVER_NOT_EXPORTED);
        else context.registerReceiver(receiver, new IntentFilter(action));
        int flags = PendingIntent.FLAG_CANCEL_CURRENT;
        if (Build.VERSION.SDK_INT >= 31) flags |= PendingIntent.FLAG_MUTABLE;
        PendingIntent pending = PendingIntent.getBroadcast(context, 9159,
            new Intent(action).setPackage(context.getPackageName()), flags);
        Throwable primaryFailure = null;
        int restoreRotation = Integer.MIN_VALUE;
        int restoreUserRotation = Integer.MIN_VALUE;
        try {
            if (scenario.equals("offline-rotation")) {
                restoreUserRotation = android.provider.Settings.System.getInt(context.getContentResolver(),
                    android.provider.Settings.System.USER_ROTATION, displayRotation());
                boolean automatic = android.provider.Settings.System.getInt(context.getContentResolver(),
                    android.provider.Settings.System.ACCELEROMETER_ROTATION, 0) == 1;
                restoreRotation = automatic ? UiAutomation.ROTATION_UNFREEZE : restoreUserRotation;
                // 浮动语音窗口沿用底层方向；用可旋转的合成调用方，避免启动器锁竖屏。
                Intent editor = new Intent().setClassName("app.msime.android.test",
                    "app.msime.android.test.EditorActivity")
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
                instrumentation.runOnMainSync(() -> context.startActivity(editor));
                // 自动化连接会保存实际方向；固定模式先让底层落实保存值，再建立连接。
                int savedRotation = restoreUserRotation;
                if (!automatic) await(() -> displayRotation() == savedRotation,
                    "Synthetic caller did not reach the saved fixed rotation");
            }
            if (scenario.equals("offline-permission-home") || scenario.equals("offline-permission-deny") || reconstruction) {
                var automation = instrumentation.getUiAutomation();
                var info = automation.getServiceInfo();
                info.flags |= android.accessibilityservice.AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
                    | android.accessibilityservice.AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS;
                automation.setServiceInfo(info);
            }
            if (scenario.equals("offline-rotation")) {
                await(this::syntheticCallerVisible, "Rotatable synthetic caller did not become visible");
            }
            Bundle extras = new Bundle();
            extras.putString("synthetic_marker", "preserved");
            extras.putStringArrayList(RecognizerIntent.EXTRA_RESULTS,
                new java.util.ArrayList<>(java.util.List.of("synthetic stale transcript")));
            Intent request = new Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH)
                .setPackage(context.getPackageName())
                .putExtra(RecognizerIntent.EXTRA_LANGUAGE, "en-US")
                .putExtra(RecognizerIntent.EXTRA_RESULTS_PENDINGINTENT, pending)
                .putExtra(RecognizerIntent.EXTRA_RESULTS_PENDINGINTENT_BUNDLE, extras)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            if (context.getPackageManager().resolveActivity(request, 0) == null) {
                throw new AssertionError("Standard request is not discoverable");
            }
            instrumentation.runOnMainSync(() -> context.startActivity(request));
            await(() -> activity != null, "External activity did not start");
            if (scenario.equals("offline-permission-home") || reconstruction) {
                await(this::permissionVisible, "Microphone permission dialog did not appear");
                instrumentation.waitForIdleSync();
                if (finished()) throw new AssertionError("Permission dialog cancelled a visible request");
                var started = activity.getClass().getDeclaredField("started");
                started.setAccessible(true);
                if (started.getBoolean(activity)) throw new AssertionError("Recording started before permission");
                if (reconstruction) {
                    Activity original = activity;
                    int originalOrientation = original.getResources().getConfiguration().orientation;
                    if (scenario.equals("offline-rotation")) {
                        int originalRotation = displayRotation();
                        int rotated = (originalRotation + 1) % 4;
                        if (!instrumentation.getUiAutomation().setRotation(rotated))
                            throw new AssertionError("System display rotation was rejected");
                        await(() -> displayRotation() == rotated, "System display did not rotate");
                    } else {
                        instrumentation.runOnMainSync(original::recreate);
                    }
                    await(original::isDestroyed, "Activity recreation did not destroy the old instance");
                    await(() -> activity != original, "System did not create a replacement activity");
                    await(this::finished, "Recreated request remained visible");
                    if (scenario.equals("offline-rotation")) instrumentation.runOnMainSync(() -> {
                        if (activity.getResources().getConfiguration().orientation == originalOrientation)
                            throw new AssertionError("Replacement activity kept the original orientation");
                    });
                    // 已销毁窗口的迟到结果不得再投递；新实例也不能重新开始录音。
                    var deliver = original.getClass().getDeclaredMethod("returnResult", String.class);
                    deliver.setAccessible(true);
                    instrumentation.runOnMainSync(() -> {
                        try { deliver.invoke(original, "synthetic stale reconstruction result"); }
                        catch (ReflectiveOperationException error) { throw new AssertionError(error); }
                    });
                    instrumentation.runOnMainSync(() -> {
                        try {
                            if (started.getBoolean(activity))
                                throw new AssertionError("Recreated activity started recording");
                        } catch (ReflectiveOperationException error) { throw new AssertionError(error); }
                    });
                } else {
                    try (var input = instrumentation.getUiAutomation().executeShellCommand("input keyevent 3")) {
                        try (var stream = new java.io.FileInputStream(input.getFileDescriptor())) {
                            byte[] discard = new byte[4096];
                            while (stream.read(discard) != -1) { }
                        }
                    }
                    await(this::finished, "Home did not cancel permission-stage request");
                }
            } else if (scenario.equals("offline-permission-deny") || scenario.equals("offline-no-model")) {
                try {
                    if (scenario.equals("offline-permission-deny"))
                        await(this::requestPermissionDenial, "System microphone deny button did not appear");
                }
                catch (AssertionError error) {
                    throw new AssertionError(error.getMessage() + "; finished=" + finished()
                        + "; permission window=" + permissionVisible() + "; system nodes=" + permissionNodes);
                }
                var failed = activity.getClass().getDeclaredField("failed");
                failed.setAccessible(true);
                AtomicBoolean visibleFailure = new AtomicBoolean();
                await(() -> {
                    instrumentation.runOnMainSync(() -> {
                        try { visibleFailure.set(failed.getBoolean(activity)); }
                        catch (ReflectiveOperationException error) { throw new AssertionError(error); }
                    });
                    return visibleFailure.get();
                }, "Permission denial did not show the product failure state");
                instrumentation.runOnMainSync(() -> {
                    try {
                        var started = activity.getClass().getDeclaredField("started");
                        var done = activity.getClass().getDeclaredField("done");
                        var settings = activity.getClass().getDeclaredField("settings");
                        var status = activity.getClass().getDeclaredField("status");
                        for (var field : java.util.List.of(started, done, settings, status)) field.setAccessible(true);
                        if (scenario.equals("offline-permission-deny") && context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO)
                                == android.content.pm.PackageManager.PERMISSION_GRANTED)
                            throw new AssertionError("System denial unexpectedly granted the microphone");
                        if (scenario.equals("offline-permission-deny") && started.getBoolean(activity))
                            throw new AssertionError("Recording started after denial");
                        if (((android.widget.Button) done.get(activity)).isEnabled())
                            throw new AssertionError("Denied request allows completion");
                        if (!((android.view.View) settings.get(activity)).isShown())
                            throw new AssertionError("Denied request has no settings entry");
                        if (scenario.equals("offline-no-model") && !stages.isEmpty())
                            throw new AssertionError("Unselected model reached the recognizer");
                        String expected = scenario.equals("offline-no-model") ? "安装并选用模型" : "麦克风权限";
                        if (!((android.widget.TextView) status.get(activity)).getText().toString().contains(expected))
                            throw new AssertionError("Denied request has no permission explanation");
                        if (deliveries.get() != 0 || finished())
                            throw new AssertionError("Denial returned before the user cancelled");
                        if (!clickCancel(activity.findViewById(android.R.id.content)))
                            throw new AssertionError("Product cancel button unavailable");
                    } catch (ReflectiveOperationException error) { throw new AssertionError(error); }
                });
            } else if (scenario.equals("offline-asr")) {
                finishSyntheticSpeech();
            } else if (!scenario.equals("offline-result") && !scenario.equals("offline-cancel") && !scenario.equals("offline-repeat")) {
                throw new AssertionError("Unknown offline scenario");
            }
            try {
                await(() -> deliveries.get() > 0, "No PendingIntent result");
            } catch (AssertionError timeout) {
                if (!scenario.equals("offline-asr")) throw timeout;
                throw new AssertionError(timeout.getMessage() + "; active methods=" + asrWorkerMethods());
            }
            instrumentation.waitForIdleSync();
            if (failure != null) throw new AssertionError("Activity callback failed", failure);
            if (deliveries.get() != 1) throw new AssertionError("Result delivered more than once");
            Intent data = returned.get();
            boolean success = scenario.equals("offline-result") || scenario.equals("offline-asr") || scenario.equals("offline-repeat");
            if (code.get() != (success ? Activity.RESULT_OK : Activity.RESULT_CANCELED)) {
                throw new AssertionError("Unexpected result code");
            }
            if (!"preserved".equals(data.getStringExtra("synthetic_marker"))) throw new AssertionError("Caller bundle lost");
            if (scenario.equals("offline-asr")) {
                var results = data.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS);
                if (results == null || results.size() != 1 || !results.get(0).toLowerCase(java.util.Locale.ROOT).contains("hello")) {
                    throw new AssertionError("Real local model did not return the synthetic greeting");
                }
            } else if (success) {
                if (!java.util.List.of("synthetic external transcript").equals(
                        data.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS))) {
                    throw new AssertionError("Unexpected synthetic result");
                }
            } else if (data.hasExtra(RecognizerIntent.EXTRA_RESULTS)) {
                throw new AssertionError("Cancelled request retained caller-supplied results");
            }
        } catch (Exception | AssertionError error) {
            primaryFailure = error;
            throw error;
        } finally {
            try {
                if (activity != null) instrumentation.runOnMainSync(activity::finish);
                // API 28 的权限窗口不响应返回键；只点击系统拒绝按钮，不能在清理时授予录音权限。
                boolean rotation = scenario.equals("offline-rotation");
                if (reconstruction && (rotation || permissionVisible())) {
                    // 旋转时节点可能暂时为空；底层调用方重新可见才能确认权限窗口已退出。
                    await(() -> (!permissionVisible() && (!rotation || syntheticCallerVisible())) || requestPermissionDenial(),
                        "Recreated permission window has no system deny button");
                    await(() -> !permissionVisible() && (!rotation || syntheticCallerVisible()),
                        "Recreated permission window was not dismissed");
                }
            } catch (Exception | AssertionError cleanupFailure) {
                // 主体断言失败优先保留；清理失败仍附在异常上，不能把真正的失败覆盖掉。
                if (primaryFailure != null) primaryFailure.addSuppressed(cleanupFailure);
                else {
                    primaryFailure = cleanupFailure;
                    throw cleanupFailure;
                }
            } finally {
                try {
                    // 失败时也恢复原来的自动旋转或固定方向，不把测试设置留给下一项。
                    if (restoreRotation != Integer.MIN_VALUE) {
                        var automation = instrumentation.getUiAutomation();
                        // 先恢复模式，再单独恢复保存方向，避免异步设置监听器在解冻时回写旧值。
                        boolean restored = automation.setRotation(restoreRotation);
                        boolean automatic = restoreRotation == UiAutomation.ROTATION_UNFREEZE;
                        if (!restored) throw new AssertionError("Original display rotation could not be restored");
                        shellCommand("settings put system user_rotation " + restoreUserRotation);
                        int expectedUserRotation = restoreUserRotation;
                        await(() -> android.provider.Settings.System.getInt(context.getContentResolver(),
                                android.provider.Settings.System.USER_ROTATION, expectedUserRotation) == expectedUserRotation
                            && android.provider.Settings.System.getInt(context.getContentResolver(),
                                android.provider.Settings.System.ACCELEROMETER_ROTATION, 0) == (automatic ? 1 : 0)
                            && cachedRotationRestored(automatic, expectedUserRotation),
                            "Original rotation settings were not restored");
                    }
                } catch (Exception | AssertionError rotationFailure) {
                    if (primaryFailure != null) primaryFailure.addSuppressed(rotationFailure);
                    else {
                        primaryFailure = rotationFailure;
                        throw rotationFailure;
                    }
                } finally {
                    pending.cancel();
                    context.unregisterReceiver(receiver);
                    if (scenario.equals("offline-rotation")) {
                        try {
                            shellCommand("am force-stop app.msime.android.test");
                        } catch (Exception | AssertionError callerCleanupFailure) {
                            if (primaryFailure != null) primaryFailure.addSuppressed(callerCleanupFailure);
                            else throw callerCleanupFailure;
                        }
                    }
                }
            }
        }
    }

    /** 等待设备命令结束；设置是否恢复仍由读取系统值的断言确认。 */
    private void shellCommand(String command) throws java.io.IOException {
        try (var input = instrumentation.getUiAutomation().executeShellCommand(command);
                var stream = new java.io.FileInputStream(input.getFileDescriptor())) {
            byte[] discard = new byte[4096];
            while (stream.read(discard) != -1) { }
        }
    }

    /** 系统设置监听异步更新缓存；自动化断开会再次解冻，必须等窗口管理器消费原值。 */
    private boolean cachedRotationRestored(boolean automatic, int rotation) {
        String expected = "mUserRotationMode=" + (automatic ? "USER_ROTATION_FREE" : "USER_ROTATION_LOCKED")
            + " mUserRotation=ROTATION_" + (rotation * 90);
        boolean restored = false;
        try (var input = instrumentation.getUiAutomation().executeShellCommand("dumpsys window");
                var stream = new java.io.FileInputStream(input.getFileDescriptor());
                var reader = new java.io.BufferedReader(new java.io.InputStreamReader(
                    stream, java.nio.charset.StandardCharsets.UTF_8))) {
            String line;
            // 只判断固定字段，不输出窗口标题、应用名或其它系统转储内容。
            while ((line = reader.readLine()) != null) if (line.contains(expected)) restored = true;
            return restored;
        } catch (java.io.IOException error) {
            throw new java.io.UncheckedIOException(error);
        }
    }

    @SuppressWarnings("deprecation") // API 28 的辅助功能节点仍需显式回收。
    private boolean syntheticCallerVisible() {
        var root = instrumentation.getUiAutomation().getRootInActiveWindow();
        if (root == null) return false;
        try { return "app.msime.android.test".contentEquals(root.getPackageName()); }
        finally { root.recycle(); }
    }

    @SuppressWarnings("deprecation") // API 28 沿用默认显示器入口，读取系统实际方向而非测试期望值。
    private int displayRotation() {
        AtomicInteger rotation = new AtomicInteger();
        instrumentation.runOnMainSync(() -> rotation.set(
            instrumentation.getTargetContext().getSystemService(android.view.WindowManager.class)
                .getDefaultDisplay().getRotation()));
        return rotation.get();
    }

    /** 等待外部注入的合成音频经过实际麦克风、模型和界面，再点击产品完成按钮。 */
    private void finishSyntheticSpeech() throws Exception {
        var listening = activity.getClass().getDeclaredField("listening");
        listening.setAccessible(true);
        Object view = listening.get(activity);
        var level = view.getClass().getDeclaredField("level");
        level.setAccessible(true);
        var status = activity.getClass().getDeclaredField("status");
        status.setAccessible(true);
        var failed = activity.getClass().getDeclaredField("failed");
        failed.setAccessible(true);
        AtomicBoolean audible = new AtomicBoolean();
        AtomicBoolean greeting = new AtomicBoolean();
        AtomicBoolean refused = new AtomicBoolean();
        long deadline = SystemClock.uptimeMillis() + 50000;
        while (!greeting.get() && !refused.get() && SystemClock.uptimeMillis() < deadline && !finished()) {
            instrumentation.runOnMainSync(() -> {
                try {
                    refused.set(failed.getBoolean(activity));
                    if (level.getFloat(view) > .02f) audible.set(true);
                    String text = ((android.widget.TextView) status.get(activity)).getText().toString();
                    if (text.toLowerCase(java.util.Locale.ROOT).contains("hello")) greeting.set(true);
                } catch (ReflectiveOperationException error) {
                    throw new AssertionError(error);
                }
            });
            SystemClock.sleep(25);
        }
        if (failure != null) throw new AssertionError("ASR diagnostics unavailable", failure);
        if (refused.get()) throw new AssertionError("Product rejected local ASR at stages " + stages);
        if (!audible.get()) throw new AssertionError("Synthetic microphone audio did not drive the listening signal; stages " + stages);
        if (!greeting.get()) throw new AssertionError("Real model did not produce the synthetic partial greeting");
        var done = activity.getClass().getDeclaredField("done");
        done.setAccessible(true);
        instrumentation.runOnMainSync(() -> {
            try {
                android.widget.Button button = (android.widget.Button) done.get(activity);
                if (!button.isEnabled()) throw new AssertionError("Listening request cannot finish");
                button.performClick();
            } catch (ReflectiveOperationException error) {
                throw new AssertionError(error);
            }
        });
    }

    private static boolean clickCancel(android.view.View view) {
        if (view instanceof android.widget.Button button && "取消".contentEquals(button.getText())
                && button.isShown() && button.isEnabled()) return button.performClick();
        if (view instanceof android.view.ViewGroup group) {
            for (int child = 0; child < group.getChildCount(); child++) {
                if (clickCancel(group.getChildAt(child))) return true;
            }
        }
        return false;
    }

    @SuppressWarnings("deprecation") // API 28 的辅助功能节点仍需显式回收。
    private boolean requestPermissionDenial() {
        var automation = instrumentation.getUiAutomation();
        for (var window : automation.getWindows()) {
            try {
                if (requestPermissionDenialRoot(window.getRoot())) return true;
            } finally { window.recycle(); }
        }
        return requestPermissionDenialRoot(automation.getRootInActiveWindow());
    }

    @SuppressWarnings("deprecation") // 只点击系统权限窗口的拒绝按钮，不调用产品权限回调。
    private boolean requestPermissionDenialRoot(android.view.accessibility.AccessibilityNodeInfo node) {
        if (node == null) return false;
        try {
            String owner = String.valueOf(node.getPackageName());
            String id = node.getViewIdResourceName();
            if ((owner.contains("permissioncontroller") || owner.contains("packageinstaller")) && id != null) {
                permissionNodes.add(id + ":enabled=" + node.isEnabled() + ":clickable=" + node.isClickable());
            }
            if ((owner.contains("permissioncontroller") || owner.contains("packageinstaller"))
                    && id != null && id.endsWith(":id/permission_deny_button")
                    && node.isEnabled() && node.isClickable()) {
                return node.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK);
            }
            for (int child = 0; child < node.getChildCount(); child++) {
                if (requestPermissionDenialRoot(node.getChild(child))) return true;
            }
            return false;
        } finally { node.recycle(); }
    }

    private boolean finished() {
        return activity != null && (activity.isFinishing() || activity.isDestroyed());
    }

    /** 仅记录固定类的方法名，区分录音阻塞与原生解码，不记录线程名或运行时参数。 */
    private static java.util.Set<String> asrWorkerMethods() {
        var methods = new java.util.TreeSet<String>();
        for (StackTraceElement[] trace : Thread.getAllStackTraces().values()) {
            for (StackTraceElement frame : trace) {
                if (java.util.Set.of("app.msime.android.LocalAsrRecognizer", "app.msime.android.NativeClient",
                        "android.media.AudioRecord").contains(frame.getClassName())) {
                    methods.add(frame.getClassName() + "." + frame.getMethodName());
                }
            }
        }
        return methods;
    }

    @SuppressWarnings("deprecation") // API 28 的辅助功能节点仍需显式回收。
    private boolean permissionVisible() {
        var automation = instrumentation.getUiAutomation();
        boolean visible = false;
        // 权限弹窗不一定成为辅助功能的活动窗口，按实际可交互窗口检查系统包。
        for (var window : automation.getWindows()) {
            try {
                visible |= permissionRoot(window.getRoot());
            } finally {
                window.recycle();
            }
        }
        return visible || permissionRoot(automation.getRootInActiveWindow());
    }

    @SuppressWarnings("deprecation") // API 28 的辅助功能节点仍需显式回收。
    private static boolean permissionRoot(android.view.accessibility.AccessibilityNodeInfo root) {
        if (root == null) return false;
        try {
            String owner = String.valueOf(root.getPackageName());
            return owner.contains("permissioncontroller") || owner.contains("packageinstaller");
        } finally {
            root.recycle();
        }
    }

    private void captureCancellation() throws Exception {
        ClassLoader loader = instrumentation.getTargetContext().getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.LocalAsrRecognizer");
        Class<?> listener = loader.loadClass("app.msime.android.LocalAsrRecognizer$Listener");
        var capture = type.getDeclaredMethod("capture", AudioRecord.class,
            java.util.concurrent.BlockingQueue.class, AtomicBoolean.class, listener);
        capture.setAccessible(true);
        Object recognizer = type.getConstructor().newInstance();
        CountDownLatch preparing = new CountDownLatch(1);
        CountDownLatch resume = new CountDownLatch(1);
        java.util.function.Consumer<Object> diagnostics = kind -> {
            if (!kind.toString().equals("VOICE_LOCAL_CAPTURE_START")) return;
            preparing.countDown();
            try {
                if (!resume.await(5, TimeUnit.SECONDS)) throw new AssertionError("Capture barrier timed out");
            } catch (InterruptedException error) {
                Thread.currentThread().interrupt();
                throw new AssertionError(error);
            }
        };
        type.getMethod("diagnostics", java.util.function.Consumer.class).invoke(recognizer, diagnostics);
        CountingRecorder recorder = new CountingRecorder();
        AtomicReference<Throwable> error = new AtomicReference<>();
        Thread worker = new Thread(() -> {
            try {
                capture.invoke(recognizer, recorder, new LinkedBlockingQueue<short[]>(), new AtomicBoolean(), null);
            } catch (ReflectiveOperationException exception) {
                error.set(exception);
            }
        });
        try {
            worker.start();
            if (!preparing.await(5, TimeUnit.SECONDS)) throw new AssertionError("Capture did not reach start boundary");
            type.getMethod("cancel").invoke(recognizer);
        } finally {
            resume.countDown();
            worker.join(5000);
        }
        if (worker.isAlive()) throw new AssertionError("Capture did not release after cancellation");
        if (error.get() != null) throw new AssertionError("Capture failed", error.get());
        if (recorder.starts.get() != 0) throw new AssertionError("Microphone started after cancellation");
    }

    /** 真实 AudioRecord 资源配合启动计数，断言取消后连一次启动调用也没有。 */
    private static final class CountingRecorder extends AudioRecord {
        final AtomicInteger starts = new AtomicInteger();

        CountingRecorder() {
            super(MediaRecorder.AudioSource.VOICE_RECOGNITION, 16000,
                AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT,
                Math.max(32000, AudioRecord.getMinBufferSize(16000,
                    AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)));
            if (getState() != STATE_INITIALIZED) throw new AssertionError("Recorder needs microphone permission");
        }

        @Override public void startRecording() {
            starts.incrementAndGet();
        }
    }

    private static void await(java.util.function.BooleanSupplier condition, String message) {
        long deadline = SystemClock.uptimeMillis() + 10000;
        // 条件可能执行一次系统点击；成功后立即返回，不能为最终断言再次触发它。
        for (;;) {
            if (condition.getAsBoolean()) return;
            if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError(message);
            SystemClock.sleep(25);
        }
    }
}
