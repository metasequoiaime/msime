package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 「关于」和「帮助与反馈」里的设备信息：每一项怎么显示，以及「复制设备信息」复制出去的文本（#5662）。
 *
 * <p>只有反馈问题时排错要用的基础信息：应用版本、品牌、型号、系统版本、处理器架构、屏幕分辨率、存储空间、运行内存、Android System WebView 版本和键盘是否已启用。没有账号、输入内容、安装的其他应用或任何标识这台设备的 id。读系统的那一半在 {@code home/DeviceInfo}，这里只做格式化，不依赖 Android。
 */
public final class DeviceInfoReport {
    /** 读不到的项显示成这样，而不是空着。 */
    public static final String UNKNOWN = "未知";

    /** 一项：标签和值。 */
    public record Entry(String label, String value) {}

    private DeviceInfoReport() {}

    /** 复制出去的文本：第一行是标题，之后每项一行「标签：值」。 */
    public static String text(String title, List<Entry> entries) {
        StringBuilder text = new StringBuilder(title);
        for (Entry entry : entries) {
            text.append('\n').append(entry.label()).append('：').append(valueOrUnknown(entry.value()));
        }
        return text.toString();
    }

    public static String valueOrUnknown(String value) {
        return value == null || value.isBlank() ? UNKNOWN : value.trim();
    }

    /** 「0.2.2（1234）」，带版本时加「 · 五笔版」之类的说明；`code` 为负表示读不到。 */
    public static String appVersion(String name, long code, String edition) {
        if (name == null || name.isBlank()) return UNKNOWN;
        StringBuilder text = new StringBuilder(name.trim());
        if (code >= 0) text.append('（').append(code).append('）');
        if (edition != null && !edition.isBlank()) text.append(" · ").append(edition.trim());
        return text.toString();
    }

    /** 「Android 11（API 30）」。 */
    public static String android(String release, int sdk) {
        String version = release == null || release.isBlank() ? UNKNOWN : release.trim();
        return "Android " + version + "（API " + sdk + "）";
    }

    /** 支持的 ABI，首选的在前：「arm64-v8a、armeabi-v7a」。 */
    public static String abis(String[] abis) {
        if (abis == null) return UNKNOWN;
        List<String> names = new ArrayList<>(abis.length);
        for (String abi : abis) if (abi != null && !abi.isBlank()) names.add(abi.trim());
        return names.isEmpty() ? UNKNOWN : String.join("、", names);
    }

    /** 「1080 × 2400 像素 · 440 dpi」；宽高读不到时为「未知」。 */
    public static String resolution(int width, int height, int dpi) {
        if (width <= 0 || height <= 0) return UNKNOWN;
        String size = width + " × " + height + " 像素";
        return dpi > 0 ? size + " · " + dpi + " dpi" : size;
    }

    /** 「可用 23.4 GB / 共 128.0 GB」；总量读不到时为「未知」。 */
    public static String storage(long available, long total) {
        if (total <= 0) return UNKNOWN;
        return "可用 " + bytes(Math.max(0, Math.min(available, total))) + " / 共 " + bytes(total);
    }

    /** 按系统设置里的习惯用十进制单位：1 GB = 1000³ 字节。 */
    public static String bytes(long bytes) {
        if (bytes < 0) return UNKNOWN;
        double gigabytes = bytes / 1e9;
        if (gigabytes >= 1) return String.format(Locale.ROOT, "%.1f GB", gigabytes);
        return String.format(Locale.ROOT, "%.0f MB", bytes / 1e6);
    }

    /** Android System WebView 的版本：「116.0.5845.163（com.google.android.webview）」；系统没有可用的 WebView 时如实说明。 */
    public static String webView(String packageName, String versionName) {
        if (packageName == null || packageName.isBlank()) return "未找到（可能已停用）";
        String version = versionName == null || versionName.isBlank() ? UNKNOWN : versionName.trim();
        return version + "（" + packageName.trim() + "）";
    }

    /** 键盘的状态：「已启用 · 当前默认」「已启用」或「未启用」。 */
    public static String keyboard(boolean enabled, boolean isDefault) {
        if (!enabled) return "未启用";
        return isDefault ? "已启用 · 当前默认" : "已启用";
    }
}
