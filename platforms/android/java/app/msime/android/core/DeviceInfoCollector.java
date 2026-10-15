package app.msime.android;

import android.app.Activity;
import android.app.ActivityManager;
import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;
import android.graphics.Rect;
import android.os.Build;
import android.os.Environment;
import android.os.StatFs;
import android.util.DisplayMetrics;
import android.webkit.WebView;
import android.view.View;
import android.view.Display;
import android.content.ComponentName;
import android.provider.Settings;
import android.view.inputmethod.InputMethodManager;
import android.view.inputmethod.InputMethodInfo;
import app.msime.android.DeviceInfoReport.Entry;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Supplier;

/**
 * 读设备信息（#5662）：「关于」页列出来，设置页和键盘功能菜单都能一键复制，反馈问题时贴进去。显示格式和复制的文本在 {@link DeviceInfoReport}。
 *
 * <p>只读排错要用的基础信息，不读任何标识这台设备或用户的东西；什么也不上传，只在用户点「复制」时进剪贴板。
 */
public final class DeviceInfoCollector {
    private DeviceInfoCollector() {}

    /** 屏幕的真实像素尺寸和密度。要从 Activity 的窗口量：应用 Context 在 Android 11 起取 WindowManager 会被系统判为误用，所以在主线程先量好再交给工作线程。 */
    public record Screen(int width, int height, int dpi) {
        @SuppressWarnings("deprecation")
        public static Screen of(Activity activity) {
            int dpi = activity.getResources().getDisplayMetrics().densityDpi;
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                Rect bounds = activity.getWindowManager().getMaximumWindowMetrics().getBounds();
                return new Screen(bounds.width(), bounds.height(), dpi);
            }
            DisplayMetrics metrics = new DisplayMetrics();
            activity.getWindowManager().getDefaultDisplay().getRealMetrics(metrics);
            return new Screen(metrics.widthPixels, metrics.heightPixels, dpi);
        }
    }

    /** 从 IME 已附着的视图取显示器，不向应用 Context 请求窗口服务。 */
    @SuppressWarnings("deprecation")
    public static Screen screen(View view) {
        try {
            Display display = view == null ? null : view.getDisplay();
            if (display == null) return new Screen(0, 0, 0);
            DisplayMetrics metrics = new DisplayMetrics();
            display.getRealMetrics(metrics);
            return new Screen(metrics.widthPixels, metrics.heightPixels, metrics.densityDpi);
        } catch (RuntimeException unavailable) {
            return new Screen(0, 0, 0);
        }
    }

    /** 系统已启用的输入法与当前默认输入法是两个独立事实。 */
    public static boolean keyboardEnabled(Context context) {
        InputMethodManager manager = context.getSystemService(InputMethodManager.class);
        if (manager == null) return false;
        for (InputMethodInfo info : manager.getEnabledInputMethodList()) {
            if (context.getPackageName().equals(info.getPackageName())
                && "app.msime.android.MSIMEInputService".equals(info.getServiceName())) return true;
        }
        return false;
    }

    public static boolean keyboardDefault(Context context) {
        String id = Settings.Secure.getString(context.getContentResolver(), Settings.Secure.DEFAULT_INPUT_METHOD);
        ComponentName component = id == null ? null : ComponentName.unflattenFromString(id);
        return component != null && context.getPackageName().equals(component.getPackageName())
            && "app.msime.android.MSIMEInputService".equals(component.getClassName());
    }

    /** 复制文本与设置页使用相同标题和字段。 */
    public static String text(Context context, List<Entry> entries) {
        return DeviceInfoReport.text(context.getApplicationInfo().loadLabel(context.getPackageManager()) + " 设备信息", entries);
    }

    /** 逐项读；任何一项在改过系统的机型上抛异常都只让那一项显示为未知，不让整组卡在「正在读取」、复制也没有反应。 */
    public static List<Entry> collect(Context context, Screen screen) {
        List<Entry> entries = new ArrayList<>(11);
        entries.add(entry("应用版本", () -> appVersion(context)));
        entries.add(entry("品牌", () -> Build.BRAND));
        entries.add(entry("型号", () -> Build.MODEL));
        entries.add(entry("系统版本", () -> DeviceInfoReport.android(Build.VERSION.RELEASE, Build.VERSION.SDK_INT)));
        entries.add(entry("系统构建", () -> Build.DISPLAY));
        entries.add(entry("处理器架构", () -> DeviceInfoReport.abis(Build.SUPPORTED_ABIS)));
        entries.add(entry("屏幕分辨率", () -> DeviceInfoReport.resolution(screen.width(), screen.height(), screen.dpi())));
        entries.add(entry("存储空间", DeviceInfoCollector::storage));
        entries.add(entry("运行内存", () -> memory(context)));
        entries.add(entry("Android System WebView", DeviceInfoCollector::webView));
        entries.add(entry("键盘", () -> DeviceInfoReport.keyboard(keyboardEnabled(context), keyboardDefault(context))));
        return entries;
    }

    /** 系统接口是这里的边界：厂商改过的系统可能在任何一项上抛运行时异常。 */
    private static Entry entry(String label, Supplier<String> value) {
        try {
            return new Entry(label, value.get());
        } catch (RuntimeException unavailable) {
            return new Entry(label, DeviceInfoReport.UNKNOWN);
        }
    }

    private static String appVersion(Context context) {
        try {
            AppVersionPolicy.Version version = AppVersionPolicy.current(context);
            return DeviceInfoReport.appVersion(version.name(), version.code(), AppEdition.current().id());
        } catch (PackageManager.NameNotFoundException missing) {
            return DeviceInfoReport.UNKNOWN;
        }
    }

    /** 数据分区（应用和词库所在的那块）的可用与总量。 */
    private static String storage() {
        try {
            StatFs stat = new StatFs(Environment.getDataDirectory().getPath());
            return DeviceInfoReport.storage(stat.getAvailableBytes(), stat.getTotalBytes());
        } catch (IllegalArgumentException unavailable) {
            return DeviceInfoReport.UNKNOWN;
        }
    }

    private static String memory(Context context) {
        ActivityManager manager = context.getSystemService(ActivityManager.class);
        if (manager == null) return DeviceInfoReport.UNKNOWN;
        ActivityManager.MemoryInfo info = new ActivityManager.MemoryInfo();
        manager.getMemoryInfo(info);
        return info.totalMem > 0 ? DeviceInfoReport.bytes(info.totalMem) : DeviceInfoReport.UNKNOWN;
    }

    /** 系统当前选用的 WebView 实现及其版本：合包里的管理界面和别的应用里的网页输入框都靠它，网页里出问题时要对照这一项。 */
    private static String webView() {
        PackageInfo info = WebView.getCurrentWebViewPackage();
        return info == null ? DeviceInfoReport.webView(null, null)
            : DeviceInfoReport.webView(info.packageName, info.versionName);
    }
}
