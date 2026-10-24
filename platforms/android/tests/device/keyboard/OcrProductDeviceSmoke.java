package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.os.SystemClock;
import android.system.Os;
import android.view.accessibility.AccessibilityNodeInfo;
import app.msime.ocr.fixture.SyntheticOcrFixture;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.RandomAccessFile;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** 只操作专用模拟器中的合成模块、相机图和编辑器，不导出任意用户内容。 */
public final class OcrProductDeviceSmoke extends DeviceSmoke {
    private Class<?> runtime;
    private Method open;
    private Method install;
    private File source;
    private boolean permissionDenied;
    private boolean prepareHandwriting;
    private boolean handwritingCoexistence;
    private HostHandwriting handwriting;
    private List<?> handwritingBefore;

    @Override public void onCreate(android.os.Bundle arguments) {
        permissionDenied = arguments != null && "true".equals(arguments.getString("ocr-permission-denied"));
        prepareHandwriting = arguments != null && "true".equals(arguments.getString("ocr-handwriting-prepare"));
        handwritingCoexistence = arguments != null && "true".equals(arguments.getString("ocr-handwriting-coexistence"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return prepareHandwriting ? "host handwriting model prepared for offline OCR coexistence"
            : handwritingCoexistence ? "same host handwriting recognizer infers before and after isolated OCR offline"
            : permissionDenied ? "revoked camera permission blocks native OCR preview"
            : "pinned OCR import, readonly isolated loading, camera capture and editor insertion";
    }

    @Override protected void runChecks() throws Exception {
        try {
            if (prepareHandwriting) {
                stage = "prepare host handwriting model";
                handwriting = new HostHandwriting(getTargetContext());
                handwriting.prepare();
            } else if (handwritingCoexistence) moduleChecks();
            else if (permissionDenied) permissionDeniedChecks();
            else productChecks();
        }
        catch (Exception | AssertionError error) {
            try { screenshot(prepareHandwriting ? "failure-handwriting-model"
                : handwritingCoexistence ? "failure-handwriting-coexistence" : "failure"); }
            catch (Exception ignored) { }
            throw error;
        }
        finally { if (handwriting != null) handwriting.close(); }
    }

    /** 撤权会终止目标进程，由脚本在两次 instrumentation 之间撤回权限。 */
    private void permissionDeniedChecks() throws Exception {
        stage = "camera permission revoked";
        check(getTargetContext().checkSelfPermission("android.permission.CAMERA")
            != android.content.pm.PackageManager.PERMISSION_GRANTED, "camera permission is revoked");
        restartIme();
        editor();
        tap(key("更多"));
        showTool("拍照识字");
        tap(tool("拍照识字"));
        awaitAny(node -> equalsText("请先在拍照识字设置中授权相机并导入配套模块。", node.getText()));
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            check(findAny(window.getRoot(), described("拍照识字相机预览")) == null,
                "revoked permission cannot create camera preview");
        screenshot("permission");
    }

    private void moduleChecks() throws Exception {
        Context context = getTargetContext();
        stage = "offline prerequisite";
        long offlineDeadline = SystemClock.uptimeMillis() + 10000;
        android.net.ConnectivityManager connectivity = context.getSystemService(android.net.ConnectivityManager.class);
        while (connectivity.getActiveNetwork() != null && SystemClock.uptimeMillis() < offlineDeadline) SystemClock.sleep(100);
        check(connectivity.getActiveNetwork() == null, "product OCR test runs without an active network");
        stage = "clear prior synthetic module";
        File previous = new File(context.getFilesDir(), "bootstrap/state/resource-packs/ocr-module-android");
        if (previous.exists()) {
            File[] entries = previous.listFiles();
            check(entries != null, "synthetic module directory readable");
            for (File entry : entries) {
                check(entry.getName().equals("msime-ocr-v1.apk") || entry.getName().equals("msime-model.json"),
                    "only known synthetic module files can be removed");
                check(entry.delete(), "remove prior synthetic module file");
            }
            check(previous.delete(), "remove prior synthetic module directory");
        }
        ClassLoader loader = context.getClassLoader();
        runtime = loader.loadClass("app.msime.android.OcrRuntime");
        open = runtime.getMethod("open", Context.class);
        Class<?> progress = loader.loadClass("app.msime.android.NativeClient$ResourcePackProgress");
        install = loader.loadClass("app.msime.android.ResourcePacks").getMethod("install",
            Context.class, String.class, List.class, List.class, progress);
        stage = "host handwriting client before OCR";
        handwritingClient();
        source = new File(context.getCacheDir(), "synthetic-ocr-import.apk");
        try (InputStream input = getContext().getAssets().open("ocr-product-module.apk");
             FileOutputStream output = new FileOutputStream(source)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
        stage = "reject missing and corrupt module";
        rejectOpen("missing module");
        corrupt(source);
        rejectImport("corrupt module");
        copySource();
        stage = "import and recognize readonly module";
        File directory = importModule();
        File apk = new File(directory, "msime-ocr-v1.apk");
        check((Os.stat(apk.getAbsolutePath()).st_mode & 0222) == 0, "import seals executable readonly");
        recognizeFixture();
        recognizeFixture();
        // 同长度错误导入不能替换已经能用的模块。
        stage = "reject replacements and tampering";
        corrupt(source);
        rejectImport("bad replacement");
        recognizeFixture();
        check(apk.setWritable(true, true), "test opens synthetic module permissions");
        rejectOpen("writable executable");
        check(apk.setReadOnly(), "test reseals synthetic module");
        corrupt(apk);
        rejectOpen("tampered published bytes");
        copySource();
        importModule();
        recognizeFixture();
        source.delete();
        stage = "host handwriting client after OCR";
        handwritingClient();
    }

    private void productChecks() throws Exception {
        moduleChecks();
        Context context = getTargetContext();
        stage = "native camera preview";
        automation.grantRuntimePermission(context.getPackageName(), "android.permission.CAMERA");
        captureDiagnostic();
        restartIme();
        editor();
        camera();
        screenshot("preview");
        stage = "capture and recognize through native IME";
        tap(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
        awaitAny(node -> equalsText("上屏勾选行", node.getText()));
        screenshot("result");
        stage = "insert selected OCR lines into editor";
        tap(node -> equalsText("上屏勾选行", node.getText()));
        dropStaleNodes();
        awaitAny(field("msime-test-plain").and(node -> node.refresh() && node.getText() != null
            && node.getText().toString().replaceAll("\\s", "").contains("SYNTHETICOCR2468")));
        screenshot("editor");
        stage = "insert only selected OCR characters";
        editor();
        camera();
        tap(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
        awaitAny(node -> equalsText("上屏选中文字", node.getText()));
        AccessibilityNodeInfo text = awaitAny(node -> equalsText("android.widget.TextView", node.getClassName())
            && node.getText() != null && node.getText().toString().contains("2468")
            && node.getActionList().contains(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_SELECTION));
        int start = text.getText().toString().indexOf("2468");
        android.os.Bundle selection = new android.os.Bundle();
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start);
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, start + 4);
        check(text.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection), "select only synthetic digits");
        tap(node -> equalsText("上屏选中文字", node.getText()));
        awaitAny(field("msime-test-plain").and(node -> equalsText("2468", node.getText())));
        screenshot("selection");

        // 未拍照时隐藏键盘后再次打开，必须能重新取得相机，不能只验证拍照回调里的释放。
        stage = "release preview when keyboard hides";
        camera();
        home("hidden");
        editor();
        camera();
        screenshot("resumed");
        // 识别后离开输入框不能把待选结果带入下一个编辑器。
        stage = "discard results after editor switch";
        tap(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
        home("switched");
        editor();
        SystemClock.sleep(2000);
        dropStaleNodes();
        AccessibilityNodeInfo target = awaitAny(field("msime-test-plain"));
        // 空 EditText 的无障碍文本可能是提示词，必须用提示标志区分真实输入。
        check(target.getText() == null || target.getText().length() == 0
            || (target.isShowingHintText() && equalsText("Plain editor", target.getHintText())
                && equalsText("Plain editor", target.getText())),
            "late recognition cannot insert in another editor");
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows())
            check(findAny(window.getRoot(), described("拍照识字面板")) == null,
                "late recognition cannot restore OCR panel in another editor");
    }

    /** HOME 请求异步切换任务，确认桌面已获得焦点且键盘消失后才能重开编辑器。 */
    private void home(String checkpoint) throws Exception {
        String launcher;
        // 通过测试的 shell 身份解析桌面，避免依赖产品进程的包可见性权限。
        try (InputStream input = new android.os.ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand("cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME"))) {
            java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[256];
            int count;
            while ((count = input.read(buffer)) != -1) bytes.write(buffer, 0, count);
            String output = new String(bytes.toByteArray(), java.nio.charset.StandardCharsets.UTF_8).trim();
            String component = output.substring(output.lastIndexOf('\n') + 1).trim();
            check(component.indexOf('/') > 0, "dedicated AVD has a resolved launcher");
            launcher = component.substring(0, component.indexOf('/'));
        }
        check(automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME), "request home transition");
        long deadline = SystemClock.uptimeMillis() + 15000;
        long activeSince = 0;
        do {
            dropStaleNodes();
            boolean activeHome = false, keyboard = false;
            for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
                AccessibilityNodeInfo root = window.getRoot();
                if (window.getType() == android.view.accessibility.AccessibilityWindowInfo.TYPE_INPUT_METHOD) keyboard = true;
                if (window.isActive() && root != null && equalsText(launcher, root.getPackageName())) activeHome = true;
            }
            // 无障碍窗口先于退场动画更新，等待稳定后再保存画面并重开编辑器。
            if (activeHome && !keyboard) {
                if (activeSince == 0) activeSince = SystemClock.uptimeMillis();
                if (SystemClock.uptimeMillis() - activeSince >= 500) { screenshot(checkpoint); return; }
            } else activeSince = 0;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("home transition must finish before reopening synthetic editor");
    }

    private void copySource() throws Exception {
        try (InputStream input = getContext().getAssets().open("ocr-product-module.apk");
             FileOutputStream output = new FileOutputStream(source)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }

    /** 仅导出专用 AVD 的固定相机输入，经与产品相同的 JPEG 解码和 EXIF 路径诊断裁剪。 */
    @SuppressWarnings("deprecation")
    private void captureDiagnostic() throws Exception {
        stage = "synthetic camera JPEG diagnostic";
        android.app.Activity activity = startActivitySync(new Intent().setClassName(getTargetContext(),
            "app.msime.android.OcrSetupActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        java.util.concurrent.CountDownLatch surfaceReady = new java.util.concurrent.CountDownLatch(1);
        android.view.SurfaceView[] surface = new android.view.SurfaceView[1];
        runOnMainSync(() -> {
            surface[0] = new android.view.SurfaceView(activity);
            surface[0].getHolder().addCallback(new android.view.SurfaceHolder.Callback() {
                @Override public void surfaceCreated(android.view.SurfaceHolder holder) { surfaceReady.countDown(); }
                @Override public void surfaceChanged(android.view.SurfaceHolder holder, int format, int width, int height) { }
                @Override public void surfaceDestroyed(android.view.SurfaceHolder holder) { }
            });
            activity.setContentView(surface[0]);
        });
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.OcrCamera");
        var constructor = type.getDeclaredConstructor();
        constructor.setAccessible(true);
        Object camera = constructor.newInstance();
        Method close = type.getDeclaredMethod("close");
        close.setAccessible(true);
        try {
            check(surfaceReady.await(15, java.util.concurrent.TimeUnit.SECONDS), "diagnostic surface ready");
            java.util.concurrent.CountDownLatch preview = new java.util.concurrent.CountDownLatch(1);
            java.util.concurrent.atomic.AtomicBoolean failed = new java.util.concurrent.atomic.AtomicBoolean();
            Method start = type.getDeclaredMethod("start", android.view.SurfaceHolder.class, int.class,
                java.util.function.BiConsumer.class, Runnable.class);
            start.setAccessible(true);
            int rotation = activity.getWindowManager().getDefaultDisplay().getRotation();
            start.invoke(camera, surface[0].getHolder(), rotation,
                (java.util.function.BiConsumer<Integer, Integer>) (width, height) -> preview.countDown(),
                (Runnable) () -> { failed.set(true); preview.countDown(); });
            check(preview.await(15, java.util.concurrent.TimeUnit.SECONDS) && !failed.get(), "diagnostic preview ready");
            SystemClock.sleep(500);
            java.util.concurrent.CountDownLatch captured = new java.util.concurrent.CountDownLatch(1);
            java.util.concurrent.atomic.AtomicReference<byte[]> bytes = new java.util.concurrent.atomic.AtomicReference<>();
            Method capture = type.getDeclaredMethod("capture", java.util.function.Consumer.class, Runnable.class);
            capture.setAccessible(true);
            capture.invoke(camera, (java.util.function.Consumer<byte[]>) value -> { bytes.set(value); captured.countDown(); },
                (Runnable) () -> { failed.set(true); captured.countDown(); });
            check(captured.await(15, java.util.concurrent.TimeUnit.SECONDS) && !failed.get(), "diagnostic JPEG captured");
            Method decode = loader.loadClass("app.msime.android.ImeOcr").getDeclaredMethod("decode", byte[].class);
            decode.setAccessible(true);
            Bitmap image = (Bitmap) decode.invoke(null, (Object) bytes.get());
            try (FileOutputStream output = new FileOutputStream(new File(
                    getTargetContext().getExternalFilesDir(null), "ocr-product-captured.png"))) {
                check(image.compress(Bitmap.CompressFormat.PNG, 100, output), "write synthetic camera diagnostic");
            } finally { image.recycle(); }
        } finally {
            close.invoke(camera);
            runOnMainSync(activity::finish);
        }
    }

    /** 共存专项保留同一识别器做前后真实推理，普通相机专项仍只检查客户端创建。 */
    private void handwritingClient() throws Exception {
        if (handwritingCoexistence) {
            if (handwriting == null) handwriting = new HostHandwriting(getTargetContext());
            List<?> recognized = handwriting.recognize();
            check(recognized.contains("中"), "host model recognizes the complete synthetic character");
            if (handwritingBefore == null) handwritingBefore = recognized;
            else check(handwritingBefore.equals(recognized), "same host recognizer keeps its inference across OCR");
            return;
        }
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.MlKitHandwritingRecognizer");
        Object client = type.getConstructor(Context.class).newInstance(getTargetContext());
        type.getMethod("close").invoke(client);
    }

    /** 只跨产品公开接口反射，不把 Google 类型或第二份 SDK 编进测试 APK。 */
    private static final class HostHandwriting {
        private final ClassLoader loader;
        private final Class<?> type;
        private final Object client;

        HostHandwriting(Context context) throws Exception {
            loader = context.getClassLoader();
            type = loader.loadClass("app.msime.android.MlKitHandwritingRecognizer");
            client = type.getConstructor(Context.class).newInstance(context);
        }

        void prepare() throws Exception {
            Class<?> listener = loader.loadClass("app.msime.android.HandwritingRecognizer$DownloadListener");
            var done = new java.util.concurrent.CountDownLatch(1);
            var success = new java.util.concurrent.atomic.AtomicBoolean();
            Object callback = Proxy.newProxyInstance(loader, new Class<?>[]{listener}, (proxy, method, values) -> {
                if (method.getName().equals("onComplete")) { success.set(true); done.countDown(); }
                if (method.getName().equals("onFailure")) done.countDown();
                return null;
            });
            type.getMethod("download", listener).invoke(client, callback);
            check(done.await(180, java.util.concurrent.TimeUnit.SECONDS) && success.get(),
                "host handwriting model download completes");
        }

        List<?> recognize() throws Exception {
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (!type.getMethod("availability").invoke(client).toString().equals("READY")) {
                check(SystemClock.uptimeMillis() < deadline, "host handwriting model ready while offline");
                SystemClock.sleep(100);
            }
            Class<?> point = loader.loadClass("app.msime.android.HandwritingInk$Point");
            var pointConstructor = point.getConstructor(float.class, float.class, long.class);
            // 与手写界面夹具相同的五笔「中」字，时间与坐标均为固定合成值。
            float[][] segments = {{88,100,88,272},{88,100,312,100},{312,100,312,272},
                {88,272,312,272},{200,40,200,360}};
            List<List<Object>> strokes = new ArrayList<>();
            long time = 1000;
            for (float[] segment : segments) {
                List<Object> stroke = new ArrayList<>();
                for (int step = 0; step <= 12; step++) {
                    float fraction = step / 12f;
                    stroke.add(pointConstructor.newInstance(segment[0] + (segment[2] - segment[0]) * fraction,
                        segment[1] + (segment[3] - segment[1]) * fraction, time));
                    time += 16;
                }
                strokes.add(stroke);
                time += 100;
            }
            Class<?> request = loader.loadClass("app.msime.android.HandwritingRecognizer$Request");
            Object ink = request.getConstructor(long.class, List.class, float.class, float.class)
                .newInstance(7L, strokes, 400f, 400f);
            Class<?> listener = loader.loadClass("app.msime.android.HandwritingRecognizer$RecognitionListener");
            var done = new java.util.concurrent.CountDownLatch(1);
            var result = new java.util.concurrent.atomic.AtomicReference<List<?>>();
            Object callback = Proxy.newProxyInstance(loader, new Class<?>[]{listener}, (proxy, method, values) -> {
                if (method.getName().equals("onResult") && Long.valueOf(7).equals(values[0])
                        && values[1] instanceof List<?> candidates) result.set(List.copyOf(candidates));
                if (method.getName().equals("onResult") || method.getName().equals("onFailure")) done.countDown();
                return null;
            });
            type.getMethod("recognize", request, listener).invoke(client, ink, callback);
            check(done.await(60, java.util.concurrent.TimeUnit.SECONDS) && result.get() != null,
                "host handwriting model returns a real inference");
            return result.get();
        }

        void close() throws Exception { type.getMethod("close").invoke(client); }
    }

    private File importModule() throws Exception {
        return (File) install.invoke(null, getTargetContext(), "ocr-module-android",
            Collections.emptyList(), Collections.singletonList(source.getAbsolutePath()), null);
    }

    private void rejectImport(String purpose) throws Exception {
        try { importModule(); }
        catch (InvocationTargetException rejected) { return; }
        throw new AssertionError("Import accepted " + purpose);
    }

    private void rejectOpen(String purpose) throws Exception {
        try {
            Object module = open.invoke(null, getTargetContext());
            runtime.getMethod("close").invoke(module);
        } catch (InvocationTargetException rejected) { return; }
        throw new AssertionError("Runtime accepted " + purpose);
    }

    private void recognizeFixture() throws Exception {
        Object module;
        try { module = open.invoke(null, getTargetContext()); }
        catch (InvocationTargetException error) {
            Throwable cause = error;
            while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
            // 仅输出异常类型和源码位置，不输出模块异常消息或识别内容。
            String location = cause.getStackTrace().length == 0 ? "" : " at " + cause.getStackTrace()[0];
            throw new AssertionError("runtime open " + cause.getClass().getName() + location);
        }
        Bitmap image = SyntheticOcrFixture.render(1);
        try {
            String[] lines = (String[]) runtime.getMethod("recognize", Bitmap.class).invoke(module, image);
            check(SyntheticOcrFixture.matches(lines), "isolated product runtime recognizes complete Chinese and Latin fixture");
        } finally {
            runtime.getMethod("close").invoke(module);
            image.recycle();
        }
    }

    private static void corrupt(File file) throws Exception {
        boolean sealed = (Os.stat(file.getAbsolutePath()).st_mode & 0222) == 0;
        if (sealed) check(file.setWritable(true, true), "test unseals synthetic bytes");
        try (RandomAccessFile output = new RandomAccessFile(file, "rw")) {
            int value = output.read();
            output.seek(0);
            output.write(value ^ 0xff);
        }
        if (sealed) check(file.setReadOnly(), "test reseals corrupt bytes");
    }

    private void editor() throws Exception {
        // 从桌面重开合成编辑器，使用测试 shell 等待启动完成，不依赖目标进程的后台启动权限。
        shellDiscard("am start -W -n app.msime.android.test/.EditorActivity -f 0x10008000");
        dropStaleNodes();
        tap(field("msime-test-plain"));
        awaitAny(key("更多"));
    }

    private void camera() throws Exception {
        dropStaleNodes();
        tap(key("更多"));
        showTool("拍照识字");
        tap(tool("拍照识字"));
        AccessibilityNodeInfo capture = await(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
        android.graphics.Rect bounds = new android.graphics.Rect();
        capture.getBoundsInScreen(bounds);
        check(bounds.width() > 0 && bounds.height() > 0, "capture control has visible touch bounds");
    }

    private void screenshot(String name) throws Exception {
        Bitmap image = automation.takeScreenshot();
        check(image != null, "synthetic product screenshot available");
        try (FileOutputStream output = new FileOutputStream(new File(
                getTargetContext().getExternalFilesDir(null), "ocr-product-" + name + ".png"))) {
            check(image.compress(Bitmap.CompressFormat.PNG, 100, output), "write synthetic screenshot");
        } finally { image.recycle(); }
    }

    private static void check(boolean value, String purpose) { if (!value) throw new AssertionError(purpose); }
}
