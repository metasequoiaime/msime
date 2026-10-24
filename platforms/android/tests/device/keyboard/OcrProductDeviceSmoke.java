package app.msime.android.test;

import android.accessibilityservice.AccessibilityService;
import android.content.ComponentName;
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
import java.util.Collections;
import java.util.List;

/** 只操作专用模拟器中的合成模块、相机图和编辑器，不导出任意用户内容。 */
public final class OcrProductDeviceSmoke extends DeviceSmoke {
    private Class<?> runtime;
    private Method open;
    private Method install;
    private File source;
    private boolean permissionDenied;

    @Override public void onCreate(android.os.Bundle arguments) {
        permissionDenied = arguments != null && "true".equals(arguments.getString("ocr-permission-denied"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return permissionDenied ? "revoked camera permission blocks native OCR preview"
            : "pinned OCR import, readonly isolated loading, camera capture and editor insertion";
    }

    @Override protected void runChecks() throws Exception {
        try { if (permissionDenied) permissionDeniedChecks(); else productChecks(); }
        catch (Exception | AssertionError error) {
            try { screenshot("failure"); } catch (Exception ignored) { }
            throw error;
        }
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

    private void productChecks() throws Exception {
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

        stage = "native camera preview";
        automation.grantRuntimePermission(context.getPackageName(), "android.permission.CAMERA");
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
        automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME);
        editor();
        camera();
        // 识别后离开输入框不能把待选结果带入下一个编辑器。
        stage = "discard results after editor switch";
        tap(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
        automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME);
        editor();
        SystemClock.sleep(2000);
        dropStaleNodes();
        AccessibilityNodeInfo target = awaitAny(field("msime-test-plain"));
        check(target.getText() == null || target.getText().length() == 0, "late recognition cannot insert in another editor");
    }

    private void copySource() throws Exception {
        try (InputStream input = getContext().getAssets().open("ocr-product-module.apk");
             FileOutputStream output = new FileOutputStream(source)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
        }
    }

    /** 仅验证已有 SDK 容器仍能创建手写客户端，不把它报告成手写模型推理验收。 */
    private void handwritingClient() throws Exception {
        Class<?> type = getTargetContext().getClassLoader().loadClass("app.msime.android.MlKitHandwritingRecognizer");
        Object client = type.getConstructor(Context.class).newInstance(getTargetContext());
        type.getMethod("close").invoke(client);
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
        getContext().startActivity(new Intent().setComponent(new ComponentName(
            "app.msime.android.test", "app.msime.android.test.EditorActivity"))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        dropStaleNodes();
        tap(field("msime-test-plain"));
        awaitAny(key("更多"));
    }

    private void camera() throws Exception {
        dropStaleNodes();
        tap(key("更多"));
        showTool("拍照识字");
        tap(tool("拍照识字"));
        awaitAny(node -> equalsText("拍照并识别", node.getText()) && node.isEnabled());
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
