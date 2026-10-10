package app.msime.android.home;

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
import androidx.fragment.app.Fragment;
import app.msime.android.AppEdition;
import app.msime.android.AppVersionPolicy;
import app.msime.android.DeviceInfoReport;
import app.msime.android.DeviceInfoReport.Entry;
import app.msime.android.R;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Supplier;

/**
 * 读设备信息（#5662）：「关于」页列出来，「关于」和「帮助与反馈」都能一键复制，反馈问题时贴进去。显示格式和复制的文本在 {@link DeviceInfoReport}。
 *
 * <p>只读排错要用的基础信息，不读任何标识这台设备或用户的东西；什么也不上传，只在用户点「复制」时进剪贴板。
 */
final class DeviceInfo {
    private DeviceInfo() {}

    /** 屏幕的真实像素尺寸和密度。要从 Activity 的窗口量：应用 Context 在 Android 11 起取 WindowManager 会被系统判为误用，所以在主线程先量好再交给工作线程。 */
    record Screen(int width, int height, int dpi) {
        @SuppressWarnings("deprecation")
        static Screen of(Activity activity) {
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

    /** 在工作线程读其余各项，回到主线程交给 `done`；页面已经离开时什么也不做。 */
    static void load(Fragment fragment, Consumer<List<Entry>> done) {
        Activity activity = fragment.getActivity();
        if (activity == null) return;
        Screen screen = Screen.of(activity);
        HostTask.run(fragment, context -> collect(context, screen), entries -> {
            if (entries != null) done.accept(entries);
        });
    }

    /** 复制出去的整段文本，标题是本版本的应用名。 */
    static String text(Context context, List<Entry> entries) {
        return DeviceInfoReport.text(context.getString(R.string.app_name) + " 设备信息", entries);
    }

    /** 读完后复制到剪贴板。 */
    static void copy(Fragment fragment) {
        load(fragment, entries -> {
            Context context = fragment.getContext();
            if (context != null) ClipboardActions.copyText(context, "设备信息", text(context, entries), "已复制设备信息");
        });
    }

    /** 逐项读；任何一项在改过系统的机型上抛异常都只让那一项显示为未知，不让整组卡在「正在读取」、复制也没有反应。 */
    static List<Entry> collect(Context context, Screen screen) {
        List<Entry> entries = new ArrayList<>(11);
        entries.add(entry("应用版本", () -> appVersion(context)));
        entries.add(entry("品牌", () -> Build.BRAND));
        entries.add(entry("型号", () -> Build.MODEL));
        entries.add(entry("系统版本", () -> DeviceInfoReport.android(Build.VERSION.RELEASE, Build.VERSION.SDK_INT)));
        entries.add(entry("系统构建", () -> Build.DISPLAY));
        entries.add(entry("处理器架构", () -> DeviceInfoReport.abis(Build.SUPPORTED_ABIS)));
        entries.add(entry("屏幕分辨率", () -> DeviceInfoReport.resolution(screen.width(), screen.height(), screen.dpi())));
        entries.add(entry("存储空间", DeviceInfo::storage));
        entries.add(entry("运行内存", () -> memory(context)));
        entries.add(entry("Android System WebView", DeviceInfo::webView));
        entries.add(entry("键盘", () -> DeviceInfoReport.keyboard(ImeSetup.enabled(context), ImeSetup.isDefault(context))));
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
