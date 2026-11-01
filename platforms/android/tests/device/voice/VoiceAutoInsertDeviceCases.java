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
        Object[] owned = new Object[1];
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
                    directory[0] = Files.createTempDirectory(context.getCacheDir().toPath(), "voice-insertion-");
                    Context isolated = isolatedContext(context, directory[0], null);
                    attachAndCreate(service, isolated);
                    Object store = storeType.getConstructor(Path.class).newInstance(Files.createDirectories(directory[0].resolve("results")));
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
                    Object entry = field(serviceType, "imeVoiceEntry").get(service);
                    checkUploadControls(entry, context, loader);
                    field(entryType, "generation").setLong(entry, 7);
                    // 签名与 ImeVoiceEntry.delivered 一致：session、text、failure、notice、pcm、elapsedMillis、language、provider。反射查找不经编译检查，产品改了参数这里要同步改。
                    Method delivered = method(entryType, "delivered", long.class, String.class, String.class,
                        String.class, byte[].class, long.class, String.class, String.class);
                    delivered.invoke(entry, 7L, "synthetic direct", null, null, null, 0L, "zh-CN", "synthetic");
                    check(editor.getText().toString().equals("synthetic direct"), "识别完成必须直接上屏");
                    delivered.invoke(entry, 6L, "stale", null, null, null, 0L, "zh-CN", "synthetic");
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
            if (failure[0] != null) throw described(failure[0]);
        } finally {
            instrumentation.runOnMainSync(() -> {
                try { destroyService(owned[0]); }
                catch (Exception error) { if (failure[0] == null) failure[0] = error; }
            });
            shutdownWorkers(owned[0]);
            deleteDirectory(directory[0]);
            if (failure[0] != null) throw described(failure[0]);
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
            Path directory = null;
            android.content.Intent[] launched = new android.content.Intent[1];
            try {
                Context context = instrumentation.getTargetContext();
                ClassLoader loader = context.getClassLoader();
                Class<?> serviceType = loader.loadClass("app.msime.android.MSIMEInputService");
                service = serviceType.getConstructor().newInstance();
                directory = Files.createTempDirectory(context.getCacheDir().toPath(), "voice-fallback-");
                attachAndCreate(service, isolatedContext(context, directory, launched));
                Class<?> entryType = loader.loadClass("app.msime.android.ImeVoiceEntry");
                entry = field(serviceType, "imeVoiceEntry").get(service);
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
                    destroyService(service);
                    shutdownWorkers(service);
                    deleteDirectory(directory);
                } catch (Exception error) { if (failure[0] == null) failure[0] = error; }
            }
        });
        if (failure[0] != null) {
            Throwable cause = failure[0];
            while (cause.getCause() != null) cause = cause.getCause();
            throw new AssertionError("HTTP 前台回退回归失败：" + cause.getClass().getSimpleName()
                + " at " + cause.getStackTrace()[0], failure[0]);
        }
    }

    /** attachBaseContext 会注册窗口配置回调，服务必须走完整创建与销毁，不能只关闭线程池。 */
    private static void attachAndCreate(Object service, Context context) throws Exception {
        method(ContextWrapper.class, "attachBaseContext", Context.class).invoke(service, context);
        service.getClass().getMethod("onCreate").invoke(service);
        // 在安装假 session 之前重放配置回调，旧的半初始化服务会在这里确定性失败。
        service.getClass().getMethod("onConfigurationChanged", android.content.res.Configuration.class)
            .invoke(service, new android.content.res.Configuration(context.getResources().getConfiguration()));
    }

    private static Context isolatedContext(Context context, Path root,
                                            android.content.Intent[] launched) throws Exception {
        Path files = Files.createDirectories(root.resolve("files"));
        Path cache = Files.createDirectories(root.resolve("cache"));
        Path state = Files.createDirectories(files.resolve("bootstrap/state"));
        Class<?> nativeType = context.getClassLoader().loadClass("app.msime.android.NativeClient");
        JSONObject defaults = new JSONObject((String) nativeType.getMethod("defaultPreferences").invoke(null));
        check(defaults.getBoolean("ok"), "合成偏好必须可用");
        JSONObject preferences = defaults.getJSONObject("value").put("usage_reporting", false);
        Files.write(state.resolve("preferences.json"), new JSONObject().put("format_version", 1)
            .put("revision", 0).put("preferences", preferences).toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
        Files.write(files.resolve("runtime-options.json"), new JSONObject()
            .put("preferences_directory", state.toString()).toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
        return new ContextWrapper(context) {
            @Override public java.io.File getFilesDir() { return files.toFile(); }
            @Override public java.io.File getCacheDir() { return cache.toFile(); }
            @Override public Context getApplicationContext() { return this; }
            @Override public void startActivity(android.content.Intent intent) {
                if (launched == null) throw new AssertionError("自动插入用例不能启动外部窗口");
                launched[0] = intent;
            }
        };
    }

    private static void destroyService(Object service) throws Exception {
        if (service == null) return;
        // 1 只是自动插入的合成身份，不能交给 NativeClient.destroy 当成真实句柄释放。
        field(service.getClass(), "session").setLong(service, 0);
        field(service.getClass(), "connection").set(service, null);
        service.getClass().getMethod("onDestroy").invoke(service);
    }

    private static void shutdownWorkers(Object service) throws Exception {
        if (service == null) return;
        for (Field member : service.getClass().getDeclaredFields()) {
            if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
            member.setAccessible(true);
            ExecutorService worker = (ExecutorService) member.get(service);
            if (worker == null) continue;
            worker.shutdownNow();
            check(worker.awaitTermination(3, java.util.concurrent.TimeUnit.SECONDS), "合成服务工作线程必须退出");
        }
    }

    private static void deleteDirectory(Path directory) throws Exception {
        if (directory == null) return;
        try (var paths = Files.walk(directory)) {
            for (Path path : paths.sorted(java.util.Comparator.reverseOrder()).toArray(Path[]::new))
                Files.deleteIfExists(path);
        }
    }

    /**
     * 本类自己的检查失败原样抛出，消息是固定的测试文字；其他异常（反射找不到成员、产品代码抛出的异常）只报每层的类型和本类里的出错行，不带异常消息。
     *
     * <p>以前一律包成「语音自动插入回归失败」，CI 上只看得到 `AssertionError`：2026-10-29 产品给 `delivered` 加了参数，这里的反射查找抛 `NoSuchMethodException`，看起来却和断言失败一样。
     */
    private static AssertionError described(Throwable failure) {
        if (failure instanceof AssertionError assertion) return assertion;
        StringBuilder message = new StringBuilder("语音自动插入回归失败：");
        Throwable root = failure;
        for (Throwable layer = failure; layer != null; layer = layer.getCause()) {
            if (layer != failure) message.append(" <- ");
            message.append(layer.getClass().getSimpleName());
            root = layer;
        }
        String self = VoiceAutoInsertDeviceCases.class.getName();
        String separator = " at ";
        for (StackTraceElement frame : root.getStackTrace()) {
            if (!frame.getClassName().startsWith(self)) continue;
            message.append(separator).append(frame.getMethodName()).append(':').append(frame.getLineNumber());
            separator = ", ";
        }
        return new AssertionError(message.toString(), failure);
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
