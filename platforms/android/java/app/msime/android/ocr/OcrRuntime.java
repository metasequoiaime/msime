package app.msime.android;

import android.content.Context;
import android.content.ContextWrapper;
import android.content.pm.ApplicationInfo;
import android.content.res.AssetManager;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.os.Build;
import android.system.Os;
import dalvik.system.DexClassLoader;
import java.io.File;
import java.lang.reflect.Method;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import org.json.JSONObject;

/** 输入法只读使用已校验的项目模块；不安装 APK，不解压文件，也不共享宿主的 Google 单例。 */
public final class OcrRuntime implements AutoCloseable {
    private static final String APK = "msime-ocr-v1.apk";
    private static Class<?> cachedClass;
    private static Context cachedContext;
    private static String cachedPath;
    private final Object module;
    private final Method recognize;
    private final Method close;

    /** 哈希整个模块并初始化识别器，只能在工作线程上调用。 */
    public static synchronized OcrRuntime open(Context context) throws Exception {
        if (AppVersionPolicy.playDistribution()) throw new IllegalStateException("ocr_distribution");
        JSONObject response = new JSONObject(NativeClient.resourcePackVerify(new JSONObject()
            .put("state_root", ResourcePacks.stateRoot(context.getFilesDir()).getAbsolutePath())
            .put("pack", ResourcePacks.OCR_MODULE).toString()));
        if (!response.optBoolean("ok")) throw new IllegalStateException("ocr_module_unavailable");
        File apk = new File(response.getJSONObject("value").getString("path"), APK);
        if ((Os.stat(apk.getAbsolutePath()).st_mode & 0222) != 0)
            throw new IllegalStateException("ocr_module_not_readonly");
        if (cachedClass == null || !apk.getAbsolutePath().equals(cachedPath)) {
            String abi = null;
            for (String supported : Build.SUPPORTED_ABIS) {
                if (supported.equals("arm64-v8a") || supported.equals("x86_64")) { abi = supported; break; }
            }
            if (abi == null) throw new IllegalStateException("ocr_module_abi");
            String libraries = apk.getAbsolutePath() + "!/lib/" + abi;
            boolean found = false;
            try (ZipFile zip = new ZipFile(apk)) {
                var entries = zip.entries();
                while (entries.hasMoreElements()) {
                    ZipEntry entry = entries.nextElement();
                    if (!entry.getName().startsWith("lib/" + abi + "/") || !entry.getName().endsWith(".so")) continue;
                    if (entry.getMethod() != ZipEntry.STORED) throw new IllegalStateException("ocr_module_packaging");
                    found = true;
                }
            }
            if (!found) throw new IllegalStateException("ocr_module_abi");
            DexClassLoader loader = new DexClassLoader(apk.getAbsolutePath(),
                context.getCodeCacheDir().getAbsolutePath(), libraries, context.getClassLoader().getParent());
            ApplicationInfo info = new ApplicationInfo(context.getApplicationInfo());
            info.sourceDir = apk.getAbsolutePath();
            info.publicSourceDir = apk.getAbsolutePath();
            info.nativeLibraryDir = libraries;
            Resources resources = context.getPackageManager().getResourcesForApplication(info);
            Context application = context.getApplicationContext();
            Context isolated = new ContextWrapper(application) {
                @Override public Context getApplicationContext() { return this; }
                @Override public ClassLoader getClassLoader() { return loader; }
                @Override public ApplicationInfo getApplicationInfo() { return info; }
                @Override public Resources getResources() { return resources; }
                @Override public AssetManager getAssets() { return resources.getAssets(); }
            };
            Class<?> type = loader.loadClass("app.msime.ocr.module.OcrModule");
            cachedClass = type;
            cachedContext = isolated;
            cachedPath = apk.getAbsolutePath();
        }
        return new OcrRuntime(cachedClass, cachedContext);
    }

    private OcrRuntime(Class<?> type, Context context) throws Exception {
        module = type.getConstructor().newInstance();
        close = type.getMethod("close");
        recognize = type.getMethod("recognize", Bitmap.class);
        if (!Integer.valueOf(1).equals(type.getMethod("interfaceVersion").invoke(module)))
            throw new IllegalStateException("ocr_module_interface");
        try { type.getMethod("initialize", Context.class).invoke(module, context); }
        catch (Exception | LinkageError error) { close(); throw error; }
    }

    /** 结果仅交给当前面板；不记录文本和模块返回的异常消息。 */
    public String[] recognize(Bitmap bitmap) throws Exception {
        return (String[]) recognize.invoke(module, bitmap);
    }

    @Override public void close() {
        try { close.invoke(module); }
        catch (ReflectiveOperationException | RuntimeException | LinkageError ignored) { }
    }
}
