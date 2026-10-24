package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Bundle;
import android.os.Parcel;
import android.os.ResultReceiver;
import android.text.InputType;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.LinearLayout;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.EditText;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import org.json.JSONObject;

/** 在真实 Android 编辑控件上验证键盘直插及窗口编号回传，不访问麦克风、网络或用户资料。 */
final class VoiceAutoInsertDeviceCases {
    private VoiceAutoInsertDeviceCases() {}

    static void run(Instrumentation instrumentation) throws Exception {
        Throwable[] failure = new Throwable[1];
        Object[] owned = new Object[2];
        Path[] directory = new Path[1];
        java.util.concurrent.CountDownLatch inserted = new java.util.concurrent.CountDownLatch(1);
        Runnable[] verify = new Runnable[1];
        try {
            instrumentation.runOnMainSync(() -> {
                try {
                    Context context = instrumentation.getTargetContext();
                    ClassLoader loader = context.getClassLoader();
                    Class<?> serviceType = loader.loadClass("app.msime.android.MSIMEInputService");
                    Class<?> storeType = loader.loadClass("app.msime.android.VoiceResultStore");
                    Class<?> snapshotType = loader.loadClass("app.msime.android.EditorContextSnapshot");
                    Object service = serviceType.getConstructor().newInstance();
                    owned[0] = service;
                    Class<?> gateType = loader.loadClass("app.msime.android.ImePrivacyGate");
                    var gateConstructor = gateType.getDeclaredConstructor(serviceType);
                    gateConstructor.setAccessible(true);
                    field(serviceType, "imePrivacyGate").set(service, gateConstructor.newInstance(service));
                    // 无统计目录，避免异步任务写入测试包以外的语音或打字记录。
                    Context isolated = new ContextWrapper(context) {
                        @Override public java.io.File getFilesDir() { return null; }
                    };
                    method(ContextWrapper.class, "attachBaseContext", Context.class).invoke(service, isolated);
                    directory[0] = Files.createTempDirectory(context.getCacheDir().toPath(), "voice-insertion-");
                    Object store = storeType.getConstructor(Path.class).newInstance(directory[0]);
                    field(serviceType, "voiceResultStore").set(service, store);
                    field(serviceType, "session").setLong(service, 1);
                    field(serviceType, "view").set(service, new JSONObject());
                    EditText editor = new EditText(context);
                    editor.setInputType(InputType.TYPE_CLASS_TEXT);
                    InputConnection connection = editor.onCreateInputConnection(new EditorInfo());
                    check(connection != null, "合成编辑器必须提供连接");
                    field(serviceType, "connection").set(service, connection);
                    method(serviceType, "captureVoiceTarget").invoke(service);
                    Class<?> entryType = loader.loadClass("app.msime.android.ImeVoiceEntry");
                    var constructor = entryType.getDeclaredConstructor(serviceType);
                    constructor.setAccessible(true);
                    Object entry = constructor.newInstance(service);
                    owned[1] = entry;
                    checkUploadControls(entry, context, loader);
                    field(entryType, "generation").setLong(entry, 7);
                    Method delivered = method(entryType, "delivered", long.class, String.class, String.class,
                        byte[].class, long.class, String.class, String.class);
                    delivered.invoke(entry, 7L, "synthetic direct", null, null, 0L, "zh-CN", "synthetic");
                    check(editor.getText().toString().equals("synthetic direct"), "识别完成必须直接上屏");
                    delivered.invoke(entry, 6L, "stale", null, null, 0L, "zh-CN", "synthetic");
                    check(editor.getText().toString().equals("synthetic direct"), "旧识别不能上屏");
                    field(serviceType, "voiceActivityGeneration").setLong(service, 10);
                    ResultReceiver receiver = (ResultReceiver) method(serviceType,
                        "voiceActivityResultReceiver", long.class).invoke(service, 10L);
                    check(receiver != null, "键盘启动窗口时必须建立结果回传");
                    method(serviceType, "closeVoiceResult").invoke(service);
                    Object saved = storeType.getMethod("save", String.class, long.class)
                        .invoke(store, " synthetic window", System.currentTimeMillis());
                    UUID id = (UUID) saved.getClass().getMethod("id").invoke(saved);
                    Bundle payload = new Bundle();
                    payload.putString("result_id", id.toString());
                    editor.addTextChangedListener(new android.text.TextWatcher() {
                        @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}
                        @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}
                        @Override public void afterTextChanged(android.text.Editable text) {
                            if (text.toString().equals("synthetic direct synthetic window")) inserted.countDown();
                        }
                    });
                    // 强制经 Parcel/Binder 传递，覆盖真实跨进程的结果编号回传。
                    Parcel parcel = Parcel.obtain();
                    try {
                        receiver.writeToParcel(parcel, 0);
                        parcel.setDataPosition(0);
                        ResultReceiver.CREATOR.createFromParcel(parcel).send(android.app.Activity.RESULT_OK, payload);
                    } finally { parcel.recycle(); }
                    verify[0] = () -> {
                        try {
                            check(storeType.getMethod("read", long.class).invoke(store, System.currentTimeMillis()) == null,
                                "自动插入后必须清除交接结果");
                            Method insert = method(serviceType, "insertVoiceActivityResult", long.class,
                                snapshotType, UUID.class, int.class);
                            insert.invoke(service, 10L, null, id, 0);
                            check(editor.getText().toString().equals("synthetic direct synthetic window"),
                                "重复回传不能重复插入");
                            Object snapshot = snapshotType.getConstructor(Object.class, long.class, String.class,
                                String.class, String.class).newInstance(connection, 0L,
                                    editor.getText().toString(), null, "");
                            Object pending = storeType.getMethod("save", String.class, long.class)
                                .invoke(store, "synthetic retained", System.currentTimeMillis());
                            UUID pendingId = (UUID) pending.getClass().getMethod("id").invoke(pending);
                            editor.setSelection(0);
                            field(serviceType, "voiceActivityGeneration").setLong(service, 20);
                            insert.invoke(service, 20L, snapshot, pendingId, 0);
                            check(editor.getText().toString().equals("synthetic direct synthetic window"),
                                "输入位置变化时不能自动插入");
                            check(storeType.getMethod("read", long.class).invoke(store, System.currentTimeMillis()) != null,
                                "输入位置变化时必须保留结果");
                        } catch (Throwable error) { failure[0] = error; }
                    };
                } catch (Throwable error) { failure[0] = error; inserted.countDown(); }
            });
            check(inserted.await(3, java.util.concurrent.TimeUnit.SECONDS), "窗口结果必须自动填入原输入框");
            instrumentation.waitForIdleSync();
            if (verify[0] != null && failure[0] == null) instrumentation.runOnMainSync(verify[0]);
            if (failure[0] != null) throw new AssertionError("语音自动插入回归失败", failure[0]);
        } finally {
            for (Object owner : owned) {
                if (owner == null) continue;
                for (Field member : owner.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                    member.setAccessible(true);
                    ((ExecutorService) member.get(owner)).shutdownNow();
                }
            }
            if (directory[0] != null) {
                try (var files = Files.list(directory[0])) {
                    for (Path path : (Iterable<Path>) files::iterator) Files.deleteIfExists(path);
                }
                Files.deleteIfExists(directory[0]);
            }
        }
    }

    /** 点击真实按钮结束采集，不能取消转写；收起面板后必须恢复原键行。 */
    private static void checkUploadControls(Object entry, Context context,
                                            ClassLoader loader) throws Exception {
        Class<?> entryType = entry.getClass();
        Class<?> uploadType = loader.loadClass("app.msime.android.HttpAsrRecognizer");
        Object upload = uploadType.getConstructor().newInstance();
        LinearLayout keys = new LinearLayout(context);
        keys.setOrientation(LinearLayout.VERTICAL);
        Button space = new Button(context);
        space.setText("synthetic space");
        keys.addView(space, new LinearLayout.LayoutParams(600, 600));
        keys.layout(0, 0, 600, 600);
        method(entryType, "show", ViewGroup.class).invoke(entry, keys);
        keys.measure(View.MeasureSpec.makeMeasureSpec(600, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(600, View.MeasureSpec.EXACTLY));
        keys.layout(0, 0, 600, 600);
        Button done = (Button) field(entryType, "finishButton").get(entry);
        check(done != null && done.getHeight() > 0 && done.getWidth() > 0 && done.isEnabled(),
            "所有识别方式共用可点击的说完了按钮");
        check("说完了".contentEquals(done.getText()), "结束采集不能写成提交结果");
        View listening = (View) field(entryType, "listening").get(entry);
        check(!listening.hasOnClickListeners(), "点击正文或空白不能丢弃录音");
        check(done.getTop() >= 0 && done.getBottom() <= 600, "完成按钮必须在键区范围内");
        check(space.getVisibility() == View.INVISIBLE, "聆听时原空格键被遮住");
        field(entryType, "upload").set(entry, upload);
        field(entryType, "recognizing").setBoolean(entry, true);
        field(entryType, "generation").setLong(entry, 5);
        done.performClick();
        check(((java.util.concurrent.atomic.AtomicBoolean) field(uploadType, "stopped").get(upload)).get(),
            "点击完成必须结束采集");
        check(!((java.util.concurrent.atomic.AtomicBoolean) field(uploadType, "cancelled").get(upload)).get(),
            "点击完成不能取消转写");
        check(!done.isEnabled(), "等待转写时不能重复提交完成");
        Method refused = method(entryType, "uploadPermissionFailed", long.class);
        refused.invoke(entry, 4L);
        check(field(entryType, "generation").getLong(entry) == 5, "旧权限失败回调不能影响当前识别");
        refused.invoke(entry, 5L);
        check(field(entryType, "generation").getLong(entry) == 6,
            "主动结束后的权限失败只能结束当前识别");
        check(space.getVisibility() == View.VISIBLE && keys.getChildCount() == 1,
            "结束后必须移除整个聆听面板并恢复键行");
        method(entryType, "show", ViewGroup.class).invoke(entry, keys);
        Object cancelled = uploadType.getConstructor().newInstance();
        field(entryType, "upload").set(entry, cancelled);
        field(entryType, "recognizing").setBoolean(entry, true);
        ViewGroup panel = (ViewGroup) field(entryType, "listeningPanel").get(entry);
        ViewGroup actions = (ViewGroup) panel.getChildAt(1);
        actions.getChildAt(1).performClick();
        check(((java.util.concurrent.atomic.AtomicBoolean) field(uploadType, "cancelled").get(cancelled)).get(),
            "点击取消必须丢弃整句识别");
        check(space.getVisibility() == View.VISIBLE && keys.getChildCount() == 1,
            "取消后必须恢复原键行");
    }

    /** 合成系统识别服务可用时验证前台回退意图，不真正启动窗口或采集音频。 */
    static void runUploadPermissionFallback(Instrumentation instrumentation) throws Exception {
        Throwable[] failure = new Throwable[1];
        instrumentation.runOnMainSync(() -> {
            Object service = null;
            Object entry = null;
            android.content.Intent[] launched = new android.content.Intent[1];
            try {
                Context context = instrumentation.getTargetContext();
                ClassLoader loader = context.getClassLoader();
                Class<?> serviceType = loader.loadClass("app.msime.android.MSIMEInputService");
                service = serviceType.getConstructor().newInstance();
                Context isolated = new ContextWrapper(context) {
                    @Override public java.io.File getFilesDir() { return null; }
                    @Override public void startActivity(android.content.Intent intent) { launched[0] = intent; }
                };
                method(ContextWrapper.class, "attachBaseContext", Context.class).invoke(service, isolated);
                Class<?> overlayType = loader.loadClass("app.msime.android.ImeDebugOverlay");
                var overlay = overlayType.getDeclaredConstructor(serviceType);
                overlay.setAccessible(true);
                field(serviceType, "imeDebugOverlay").set(service, overlay.newInstance(service));
                Class<?> entryType = loader.loadClass("app.msime.android.ImeVoiceEntry");
                var constructor = entryType.getDeclaredConstructor(serviceType);
                constructor.setAccessible(true);
                entry = constructor.newInstance(service);
                field(entryType, "generation").setLong(entry, 9L);
                method(entryType, "uploadPermissionFailed", long.class).invoke(entry, 9L);
                check(launched[0] != null && launched[0].getComponent().getClassName()
                    .equals("app.msime.android.VoiceRecognitionActivity"),
                    "HTTP 麦克风被拒绝后必须回退到前台识别窗口");
                check(field(entryType, "generation").getLong(entry) == 10L,
                    "前台窗口启动前必须使键区回调失效");
                loader.loadClass("app.msime.android.VoiceRecognitionActivity")
                    .getMethod("clearRequest", String.class).invoke(null,
                        launched[0].getStringExtra("app.msime.android.voice.REQUEST_ID"));
                launched[0] = null;
                method(entryType, "uploadPermissionFailed", long.class).invoke(entry, 9L);
                check(launched[0] == null, "过期权限回调不能重复启动窗口");
            } catch (Throwable error) { failure[0] = error; }
            finally {
                try {
                    if (entry != null) method(entry.getClass(), "shutdown").invoke(entry);
                    if (service != null) {
                        Object ownEntry = field(service.getClass(), "imeVoiceEntry").get(service);
                        if (ownEntry != null) method(ownEntry.getClass(), "shutdown").invoke(ownEntry);
                    }
                } catch (Exception error) { failure[0] = error; }
            }
        });
        if (failure[0] != null) {
            Throwable cause = failure[0];
            while (cause.getCause() != null) cause = cause.getCause();
            throw new AssertionError("HTTP 前台回退回归失败：" + cause.getClass().getSimpleName()
                + " at " + cause.getStackTrace()[0], failure[0]);
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters);
        method.setAccessible(true);
        return method;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
