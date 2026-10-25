package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Intent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.IntentFilter;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;

/** 验证真实语音窗口的启动、恢复前停止及同步拒绝路径；合成服务不访问麦克风或网络。 */
public final class VoiceRecognitionLifecycleDeviceSmoke extends Instrumentation {
    static final String CALLBACKS = "app.msime.android.test.SPEECH_READY_ERROR";
    private String scenario;
    private volatile boolean callbacksDelivered;
    private volatile String violation;
    private volatile int created;
    private volatile int resumed;
    private volatile Activity voiceActivity;
    private volatile String returnedResultId;
    private OfflineVoiceDeviceCases offline;
    private final java.util.List<Integer> diagnosticCodes = new java.util.concurrent.CopyOnWriteArrayList<>();

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        scenario = arguments == null ? "ready-error" : arguments.getString("scenario", "ready-error");
        if (scenario.startsWith("offline-")) offline = new OfflineVoiceDeviceCases(this, scenario);
        start();
    }

    @Override public void callActivityOnCreate(Activity activity, Bundle state) {
        super.callActivityOnCreate(activity, state);
        if (offline != null) offline.onCreated(activity);
        if (!isVoice(activity)) return;
        voiceActivity = activity;
        created++;
        try {
            if (recognizer(activity) != null) violation = "Recognizer started before resume";
            if (scenario.equals("result")) {
                var save = activity.getClass().getDeclaredMethod("saveResult", String.class);
                save.setAccessible(true);
                save.invoke(activity, "synthetic window transcript");
                var finish = activity.getClass().getDeclaredMethod("finishRequest");
                finish.setAccessible(true);
                finish.invoke(activity);
            }
            if (scenario.equals("stop-before-resume")) {
                activity.getClass().getMethod("stopActive").invoke(null);
                if (!activity.isFinishing()) violation = "Pending stop did not finish the request";
            }
        } catch (ReflectiveOperationException error) {
            violation = "Recognizer state unavailable";
        }
    }

    @Override public void callActivityOnResume(Activity activity) {
        super.callActivityOnResume(activity);
        if (!isVoice(activity)) return;
        resumed++;
        try {
            boolean started = recognizer(activity) != null;
            if (started != scenario.equals("ready-error")) violation = "Unexpected recognizer state after resume";
        } catch (ReflectiveOperationException error) {
            violation = "Recognizer state unavailable";
        }
    }

    private static Object recognizer(Activity activity) throws ReflectiveOperationException {
        var field = activity.getClass().getDeclaredField("recognizer");
        field.setAccessible(true);
        return field.get(activity);
    }

    private static boolean isVoice(Activity activity) {
        return activity.getClass().getName().equals("app.msime.android.VoiceRecognitionActivity");
    }

    @Override public void onStart() {
        if (offline != null) {
            offline.run();
            return;
        }
        Bundle result = new Bundle();
        boolean passed = false;
        BroadcastReceiver receiver = new BroadcastReceiver() {
            @Override public void onReceive(Context context, Intent intent) {
                callbacksDelivered = true;
            }
        };
        if (Build.VERSION.SDK_INT >= 33) {
            getTargetContext().registerReceiver(receiver, new IntentFilter(CALLBACKS), Context.RECEIVER_EXPORTED);
        } else {
            getTargetContext().registerReceiver(receiver, new IntentFilter(CALLBACKS));
        }
        try {
            if (scenario.equals("result")) VoiceAutoInsertDeviceCases.runUploadPermissionFallback(this);
            String requestId = "voice-lifecycle-" + scenario;
            Intent intent = new Intent().setClassName(getTargetContext(),
                "app.msime.android.VoiceRecognitionActivity")
                .putExtra("app.msime.android.voice.REQUEST_ID", requestId)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            android.os.ResultReceiver diagnostics = new android.os.ResultReceiver(
                    new android.os.Handler(android.os.Looper.getMainLooper())) {
                @Override protected void onReceiveResult(int code, Bundle payload) {
                    if (payload != null) violation = "Voice diagnostic contains a payload";
                    diagnosticCodes.add(code);
                }
            };
            if (scenario.equals("result")) {
                intent.putExtra("app.msime.android.voice.RESULT_RECEIVER", new android.os.ResultReceiver(
                        new android.os.Handler(android.os.Looper.getMainLooper())) {
                    @Override protected void onReceiveResult(int code, Bundle payload) {
                        if (code != Activity.RESULT_OK || payload == null || payload.size() != 1) {
                            violation = "Unexpected voice result payload";
                            return;
                        }
                        returnedResultId = payload.getString("result_id");
                    }
                });
            }
            // 经 Parcel 强制走 Binder 回传，覆盖真实进程间传递的整数事件契约。
            android.os.Parcel parcel = android.os.Parcel.obtain();
            try {
                diagnostics.writeToParcel(parcel, 0);
                parcel.setDataPosition(0);
                intent.putExtra("app.msime.android.voice.DIAGNOSTICS",
                    android.os.ResultReceiver.CREATOR.createFromParcel(parcel));
            } finally {
                parcel.recycle();
            }
            // 恢复前停止会在 onCreate 内结束窗口；异步启动避免 startActivitySync 等待一个不会恢复的窗口。
            runOnMainSync(() -> getTargetContext().startActivity(intent));
            long deadline = SystemClock.uptimeMillis() + 10000;
            boolean expectCallbacks = scenario.equals("ready-error");
            while ((voiceActivity == null || (!voiceActivity.isFinishing() && !voiceActivity.isDestroyed())
                    || (expectCallbacks && !callbacksDelivered)) && SystemClock.uptimeMillis() < deadline) {
                SystemClock.sleep(50);
            }
            waitForIdleSync();
            if (violation != null) throw new AssertionError(violation);
            if (callbacksDelivered != expectCallbacks) throw new AssertionError("Unexpected service callbacks");
            if (created != 1 || resumed > 1 || (expectCallbacks && resumed != 1))
                throw new AssertionError("Unexpected voice lifecycle count");
            if (voiceActivity == null || (!voiceActivity.isFinishing() && !voiceActivity.isDestroyed()))
                throw new AssertionError("Voice request did not finish");
            if (Boolean.TRUE.equals(voiceActivity.getClass().getMethod("isRequestActive", String.class)
                    .invoke(null, requestId))) throw new AssertionError("Finished request remains active");
            Class<?> kinds = getTargetContext().getClassLoader().loadClass("app.msime.android.DiagnosticEventKind");
            Object engine = kinds.getMethod("fromWire", String.class).invoke(null, "voice_engine_platform");
            Object failure = kinds.getMethod("fromWire", String.class).invoke(null, "voice_failure_permission");
            Object success = kinds.getMethod("fromWire", String.class).invoke(null, "voice_success");
            java.util.List<Integer> expected = scenario.equals("result")
                ? java.util.List.of(((Enum<?>) success).ordinal())
                : scenario.equals("stop-before-resume") ? java.util.List.of()
                : java.util.List.of(((Enum<?>) engine).ordinal(), ((Enum<?>) failure).ordinal());
            if (scenario.equals("result")) {
                java.util.UUID id = java.util.UUID.fromString(returnedResultId);
                Class<?> storeType = getTargetContext().getClassLoader().loadClass("app.msime.android.VoiceResultStore");
                Object store = storeType.getConstructor(java.nio.file.Path.class).newInstance(
                    getTargetContext().getFilesDir().toPath().resolve("voice-handoff"));
                Object text = storeType.getMethod("consume", java.util.UUID.class, long.class)
                    .invoke(store, id, System.currentTimeMillis());
                if (!"synthetic window transcript".equals(text))
                    throw new AssertionError("Window result ID does not identify its saved transcript");
            }
            if (!diagnosticCodes.equals(expected)) throw new AssertionError("Unexpected diagnostic sequence");
            result.putString("stream", "MSIME_VOICE_LIFECYCLE_PASSED: " + scenario + "\n");
            passed = true;
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_VOICE_LIFECYCLE_FAILED: " + scenario + " "
                + error.getClass().getSimpleName()
                + (error instanceof AssertionError ? " " + error.getMessage() : "") + "\n");
        } finally {
            getTargetContext().unregisterReceiver(receiver);
            if (voiceActivity != null) runOnMainSync(voiceActivity::finish);
        }
        finish(passed ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }
}
