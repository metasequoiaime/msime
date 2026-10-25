package app.msime.ocr.probe;

import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.content.res.AssetManager;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.os.Bundle;
import app.msime.ocr.fixture.SyntheticOcrFixture;
import com.google.mlkit.common.MlKit;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognition;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModel;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModelIdentifier;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizer;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizerOptions;
import dalvik.system.DexClassLoader;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/** 仅使用合成图的隔离运行库探针，不操作产品设置或真实编辑器。 */
public final class OcrProbe extends Instrumentation {
    private String stage = "startup";
    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runProbe();
            result.putString("stream", "MSIME_OCR_PROBE_PASSED: isolated offline Chinese/Latin OCR, vertical column order and handwriting coexist\n");
            finish(-1, result);
        } catch (Throwable error) {
            // 只记录异常类型，不输出识别文本或任意模块的异常消息。
            result.putString("stream", "MSIME_OCR_PROBE_FAILED: " + stage + " / " + failureDetails(error) + "\n");
            finish(1, result);
        }
    }

    private static String failureDetails(Throwable error) {
        while (error.getCause() != null && error.getCause() != error) error = error.getCause();
        StringBuilder details = new StringBuilder(error.getClass().getName());
        StackTraceElement[] frames = error.getStackTrace();
        for (int index = 0; index < Math.min(5, frames.length); index++) {
            details.append("\n  at ").append(frames[index]);
        }
        return details.toString();
    }

    private void runProbe() throws Exception {
        Context context = getTargetContext();
        stage = "network-permission";
        if (context.checkSelfPermission("android.permission.INTERNET") == PackageManager.PERMISSION_GRANTED)
            throw new AssertionError("probe_network_permission");
        // 宿主的自动初始化 Provider 已运行，重复 initialize 会拒绝已有容器。
        stage = "handwriting-before-ocr";
        DigitalInkRecognizer handwriting = handwritingClient();
        stage = "module-files";
        File directory = new File(context.getFilesDir(), "ocr-probe");
        if (!directory.mkdirs() && !directory.isDirectory()) throw new AssertionError("probe_directory");
        File apk = new File(directory, "module.apk");
        try (InputStream input = context.getAssets().open("ocr-module.apk");
             FileOutputStream output = new FileOutputStream(apk)) {
            // Android 14 起动态代码必须只读，在打开文件描述符后、写入内容前锁住路径。
            if (!apk.setReadOnly()) throw new AssertionError("probe_readonly_module");
            copy(input, output);
        }
        File libraries = new File(directory, "lib");
        if (!libraries.mkdirs() && !libraries.isDirectory()) throw new AssertionError("probe_libraries");
        // 模块带 arm64-v8a 和 x86_64 两套原生库，取设备首选的那一套。
        String abi = null;
        for (String supported : android.os.Build.SUPPORTED_ABIS) {
            if (supported.equals("arm64-v8a") || supported.equals("x86_64")) { abi = supported; break; }
        }
        if (abi == null) throw new AssertionError("probe_abi");
        String prefix = "lib/" + abi + "/";
        int libraryCount = 0;
        try (ZipFile zip = new ZipFile(apk)) {
            var entries = zip.entries();
            while (entries.hasMoreElements()) {
                ZipEntry entry = entries.nextElement();
                String name = entry.getName();
                if (!name.startsWith(prefix) || !name.endsWith(".so")) continue;
                libraryCount++;
                String filename = name.substring(prefix.length());
                if (filename.contains("/") || filename.contains("\\")) throw new AssertionError("probe_library_name");
                File file = new File(libraries, filename);
                try (InputStream input = zip.getInputStream(entry);
                     FileOutputStream output = new FileOutputStream(file)) {
                    copy(input, output);
                }
                if (!file.setReadOnly()) throw new AssertionError("probe_readonly_library");
            }
        }
        if (libraryCount == 0) throw new AssertionError("probe_module_abi");
        stage = "class-loader";
        DexClassLoader loader = new DexClassLoader(apk.getAbsolutePath(),
            context.getCodeCacheDir().getAbsolutePath(), libraries.getAbsolutePath(),
            context.getClassLoader().getParent());
        // 模块不带 ML Kit，父加载器是启动类加载器，宿主的手写 SDK 对模块不可见。
        try {
            loader.loadClass(MlKit.class.getName());
            throw new AssertionError("probe_module_sees_mlkit");
        } catch (ClassNotFoundException expected) { }
        ApplicationInfo info = new ApplicationInfo(context.getApplicationInfo());
        info.sourceDir = apk.getAbsolutePath();
        info.publicSourceDir = apk.getAbsolutePath();
        info.nativeLibraryDir = libraries.getAbsolutePath();
        stage = "module-resources";
        Resources resources = context.getPackageManager().getResourcesForApplication(info);
        Context moduleContext = new ContextWrapper(context) {
            @Override public Context getApplicationContext() { return this; }
            @Override public ClassLoader getClassLoader() { return loader; }
            @Override public ApplicationInfo getApplicationInfo() { return info; }
            @Override public Resources getResources() { return resources; }
            @Override public AssetManager getAssets() { return resources.getAssets(); }
        };
        Class<?> moduleClass = loader.loadClass("app.msime.ocr.module.OcrModule");
        Object module = moduleClass.getConstructor().newInstance();
        if (!Integer.valueOf(1).equals(moduleClass.getMethod("interfaceVersion").invoke(module)))
            throw new AssertionError("probe_interface_version");
        stage = "public-initialization";
        moduleClass.getMethod("initialize", Context.class).invoke(module, moduleContext);
        String[][] cases = new String[SyntheticOcrFixture.CASE_COUNT + 1][];
        int completeCases = 0;
        boolean vertical;
        try {
            for (int index = 0; index < SyntheticOcrFixture.CASE_COUNT; index++) {
                if (index > 0) {
                    stage = "module-reopen-" + index;
                    moduleClass.getMethod("close").invoke(module);
                    module = moduleClass.getConstructor().newInstance();
                    moduleClass.getMethod("initialize", Context.class).invoke(module, moduleContext);
                }
                stage = "offline-recognition-case-" + index;
                Bitmap bitmap = SyntheticOcrFixture.render(index);
                try {
                    SyntheticOcrFixture.saveImage(context, bitmap, index);
                    cases[index] = (String[]) moduleClass.getMethod("recognize", Bitmap.class).invoke(module, bitmap);
                    boolean matches = SyntheticOcrFixture.matches(cases[index]);
                    if (matches) completeCases++;
                    // 所有字号均保留匹配结果，不能只展示最好的那个字号。
                    String exact = String.join("", cases[index]).replaceAll("\\s", "");
                    android.util.Log.i("MSIME_OCR_PROBE", "synthetic_case=" + index + ",complete=" + matches
                        + ",exact-simplified=" + (exact.contains("SYNTHETICOCR2468") && exact.contains("中文测试")));
                } finally {
                    bitmap.recycle();
                }
            }
            // 竖排两列必须先读右列：评测发现上游的排序会把竖排的列序读反。
            stage = "offline-recognition-vertical";
            Bitmap columns = SyntheticOcrFixture.renderVertical();
            try {
                SyntheticOcrFixture.saveImage(context, columns, SyntheticOcrFixture.CASE_COUNT);
                cases[SyntheticOcrFixture.CASE_COUNT] = (String[]) moduleClass.getMethod("recognize", Bitmap.class)
                    .invoke(module, columns);
            } finally {
                columns.recycle();
            }
            vertical = SyntheticOcrFixture.verticalOrder(cases[SyntheticOcrFixture.CASE_COUNT]);
            android.util.Log.i("MSIME_OCR_PROBE", "synthetic_vertical_right_to_left=" + vertical);
            SyntheticOcrFixture.saveResult(context, cases);
        } finally {
            moduleClass.getMethod("close").invoke(module);
            handwriting.close();
        }
        // OCR 销毁后仍能创建原宿主的手写客户端，不能重置或替换它的组件容器。
        stage = "handwriting-after-ocr";
        handwritingClient().close();
        // 模型误识别不能让手写共存检查被跳过；完整识别仍是独立的硬条件。
        stage = "offline-recognition(complete-cases=" + completeCases + ",vertical=" + vertical + ",handwriting-after=true)";
        if (completeCases == 0) throw new AssertionError("probe_synthetic_recognition");
        if (!vertical) throw new AssertionError("probe_vertical_column_order");
    }

    private static void copy(InputStream input, OutputStream output) throws Exception {
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
    }

    private static DigitalInkRecognizer handwritingClient() throws Exception {
        var identifier = DigitalInkRecognitionModelIdentifier.fromLanguageTag("zh-Hani-CN");
        if (identifier == null) throw new AssertionError("probe_handwriting_identifier");
        var model = DigitalInkRecognitionModel.builder(identifier).build();
        return DigitalInkRecognition.getClient(DigitalInkRecognizerOptions.builder(model).build());
    }
}
