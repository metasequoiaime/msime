package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Bundle;
import android.os.Parcel;
import android.os.ResultReceiver;
import android.text.InputType;
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
