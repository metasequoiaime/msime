import app.msime.android.DeviceInfoReport;
import app.msime.android.DeviceInfoReport.Entry;
import java.util.List;

/** 「复制设备信息」的格式（#5662）：每项一行，读不到的写「未知」，单位与系统设置一致。 */
public final class DeviceInfoReportSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static void equal(String actual, String expected, String message) {
        if (!expected.equals(actual)) throw new AssertionError(message + ": expected <" + expected + "> but was <" + actual + ">");
    }

    public static void main(String[] args) {
        equal(DeviceInfoReport.appVersion("0.2.2", 1234, "full"), "0.2.2（1234） · full", "version, code and edition");
        equal(DeviceInfoReport.appVersion("0.2.2", -1, ""), "0.2.2", "unknown code and no edition");
        equal(DeviceInfoReport.appVersion(null, 1, "full"), "未知", "no version name");
        equal(DeviceInfoReport.android("11", 30), "Android 11（API 30）", "android release and api");
        equal(DeviceInfoReport.abis(new String[] {"arm64-v8a", "armeabi-v7a", " "}), "arm64-v8a、armeabi-v7a", "abis keep order");
        equal(DeviceInfoReport.abis(new String[0]), "未知", "no abi");
        equal(DeviceInfoReport.resolution(1080, 2400, 440), "1080 × 2400 像素 · 440 dpi", "resolution with density");
        equal(DeviceInfoReport.resolution(0, 2400, 440), "未知", "missing width");
        equal(DeviceInfoReport.storage(23_400_000_000L, 128_000_000_000L), "可用 23.4 GB / 共 128.0 GB", "decimal storage units");
        equal(DeviceInfoReport.storage(500_000_000L, 0), "未知", "unknown total storage");
        equal(DeviceInfoReport.bytes(512_000_000L), "512 MB", "sub-gigabyte sizes in MB");
        equal(DeviceInfoReport.webView("com.google.android.webview", "116.0.5845.163"),
            "116.0.5845.163（com.google.android.webview）", "webview version and provider");
        equal(DeviceInfoReport.webView(null, null), "未找到（可能已停用）", "webview missing");
        equal(DeviceInfoReport.keyboard(true, true), "已启用 · 当前默认", "default keyboard");
        equal(DeviceInfoReport.keyboard(false, true), "未启用", "disabled keyboard is never the default");

        String text = DeviceInfoReport.text("水杉输入法 设备信息",
            List.of(new Entry("品牌", "vivo"), new Entry("型号", "V2046A"), new Entry("系统构建", " ")));
        equal(text, "水杉输入法 设备信息\n品牌：vivo\n型号：V2046A\n系统构建：未知", "copied text is one entry per line");
        check(!text.endsWith("\n"), "no trailing newline");
        System.out.println("Android device info report passed");
    }
}
