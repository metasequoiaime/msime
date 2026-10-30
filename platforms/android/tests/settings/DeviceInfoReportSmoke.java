import app.msime.android.DeviceInfoReport;
import app.msime.android.DeviceInfoReport.Entry;
import app.msime.android.DeviceInfoReport.Field;
import app.msime.android.DeviceInfoReport.Item;
import app.msime.android.DeviceInfoReport.Language;
import app.msime.android.DeviceInfoReport.ScreenFacts;
import java.util.List;

/** 「复制设备信息」的格式（#5662）：每项一行，读不到的写「未知」，单位与系统设置一致；键盘子面板（#10045）的中英两种标签、单项插入和屏幕参数组。 */
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
        equal(text, "水杉输入法 设备信息\n用于反馈问题时定位输入法与设备兼容性；不包含账号、输入内容或设备标识。\n品牌：vivo\n型号：V2046A\n系统构建：未知", "copied text explains its purpose");
        check(!text.endsWith("\n"), "no trailing newline");
        keyboardFields();
        screen();
        System.out.println("Android device info report passed");
    }

    /** 键盘子面板：九项可单独插入，中英标签只换标签和说明，值不翻译；读不到写「未知」或「Unknown」。 */
    static void keyboardFields() {
        check(DeviceInfoReport.INSERTABLE.equals(List.of(Field.BRAND, Field.MODEL, Field.ANDROID, Field.ABIS,
            Field.STORAGE, Field.MEMORY, Field.WEBVIEW, Field.CPU, Field.DISPLAY)), "nine insertable fields in issue order");
        Item memory = new Item(Field.MEMORY, DeviceInfoReport.memory(8_000_000_000L, Language.CHINESE),
            DeviceInfoReport.memory(8_000_000_000L, Language.ENGLISH));
        equal(DeviceInfoReport.line(memory, Language.CHINESE), "运行内存：8.0 GB", "chinese single line");
        equal(DeviceInfoReport.line(memory, Language.ENGLISH), "RAM: 8.0 GB", "english single line");
        Item missing = DeviceInfoReport.item(List.of(memory), Field.CPU);
        equal(DeviceInfoReport.line(missing, Language.CHINESE), "CPU 名称：未知", "missing field is unknown");
        equal(DeviceInfoReport.line(missing, Language.ENGLISH), "CPU name: Unknown", "missing field is unknown in english");
        equal(DeviceInfoReport.line(new Item(Field.DISPLAY, null, " "), Language.ENGLISH), "Display: Unknown",
            "unknown display group stays on one line");

        equal(DeviceInfoReport.appVersion("0.2.2", 1234, "full", Language.ENGLISH), "0.2.2 (1234) · full", "english version");
        equal(DeviceInfoReport.android("11", 30, Language.ENGLISH), "Android 11 (API 30)", "english android");
        equal(DeviceInfoReport.android(null, 30, Language.ENGLISH), "Android Unknown (API 30)", "english unknown release");
        equal(DeviceInfoReport.abis(new String[] {"arm64-v8a", "armeabi-v7a"}, Language.ENGLISH), "arm64-v8a, armeabi-v7a", "english abis");
        equal(DeviceInfoReport.storage(23_400_000_000L, 128_000_000_000L, Language.ENGLISH), "23.4 GB free / 128.0 GB total", "english storage");
        equal(DeviceInfoReport.storage(1, 0, Language.ENGLISH), "Unknown", "english unknown storage");
        equal(DeviceInfoReport.memory(0, Language.CHINESE), "未知", "unknown memory");
        equal(DeviceInfoReport.webView("com.google.android.webview", "116.0", Language.ENGLISH),
            "116.0 (com.google.android.webview)", "english webview");
        equal(DeviceInfoReport.webView(null, null, Language.ENGLISH), "Not found (may be disabled)", "english missing webview");
        equal(DeviceInfoReport.keyboard(true, true, Language.ENGLISH), "Enabled · default", "english default keyboard");
        equal(DeviceInfoReport.keyboard(false, false, Language.ENGLISH), "Not enabled", "english disabled keyboard");

        // 处理器名称照系统原文：SoC 厂商加型号，型号已带厂商名时不重复；都没有时退回 Build.HARDWARE，字面的 unknown 当作没有。
        equal(DeviceInfoReport.cpu("QTI", "SM8550", "qcom"), "QTI SM8550", "soc manufacturer and model");
        equal(DeviceInfoReport.cpu("Samsung", "Samsung Exynos E1080", "s5e8825"), "Samsung Exynos E1080", "model already names maker");
        equal(DeviceInfoReport.cpu("unknown", "unknown", "mt6785"), "mt6785", "falls back to hardware");
        equal(DeviceInfoReport.cpu(null, "SM8550", null), "SM8550", "model without manufacturer");
        check(DeviceInfoReport.cpu(null, null, "unknown") == null, "nothing known");

        // 连续插入不粘连：多行框先换行，单行框先空一格，开头或已在行首时不加。
        equal(DeviceInfoReport.afterText(null, true, "品牌：vivo"), "品牌：vivo", "start of editor");
        equal(DeviceInfoReport.afterText("", false, "品牌：vivo"), "品牌：vivo", "empty before");
        equal(DeviceInfoReport.afterText("好", true, "品牌：vivo"), "\n品牌：vivo", "multi-line editor gets a new line");
        equal(DeviceInfoReport.afterText("\n", true, "品牌：vivo"), "品牌：vivo", "already at line start");
        equal(DeviceInfoReport.afterText("好", false, "品牌：vivo"), " 品牌：vivo", "single-line editor gets a space");
        equal(DeviceInfoReport.afterText(" ", false, "品牌：vivo"), "品牌：vivo", "single-line editor after a space");

        List<Item> items = List.of(new Item(Field.BRAND, "vivo", "vivo"), memory,
            new Item(Field.DISPLAY, "分辨率：1080 × 2400 像素（FHD+）", "Resolution: 1080 × 2400 pixels (FHD+)"));
        equal(DeviceInfoReport.text("水杉输入法 设备信息", items, Language.CHINESE),
            "水杉输入法 设备信息\n用于反馈问题时定位输入法与设备兼容性；不包含账号、输入内容或设备标识。\n品牌：vivo\n运行内存：8.0 GB\n屏幕参数：\n分辨率：1080 × 2400 像素（FHD+）",
            "chinese full text with display group");
        equal(DeviceInfoReport.text("Metasequoia IME device information", items, Language.ENGLISH),
            "Metasequoia IME device information\nFor locating input method and device compatibility issues in bug reports; contains no account, typed text or device identifier.\nBrand: vivo\nRAM: 8.0 GB\nDisplay:\nResolution: 1080 × 2400 pixels (FHD+)",
            "english full text with display group");
    }

    /** 屏幕参数一组：分辨率档位、ppi 与密度档、字体缩放原值、对角线英寸、常见宽高比、去重刷新率、HDR 与广色域；没有亮度。 */
    static void screen() {
        ScreenFacts facts = new ScreenFacts(1080, 2376, 480, 397f, 397f, 1.25002f,
            new float[] {60.000004f, 120f, 60f, 90f}, new int[] {2, 3}, true);
        equal(DeviceInfoReport.screen(facts, Language.CHINESE), String.join("\n",
            "分辨率：1080 × 2376 像素（FHD+）",
            "密度：397 ppi / 480 dpi（XXHDPI）",
            "字体缩放：1.25002",
            "尺寸大小：6.6 英寸",
            "宽高比：20:9",
            "刷新率：60.0 Hz、90.0 Hz、120.0 Hz",
            "HDR：支持",
            "HDR 能力：HDR10、HLG",
            "广色域：支持"), "chinese screen group");
        equal(DeviceInfoReport.screen(facts, Language.ENGLISH), String.join("\n",
            "Resolution: 1080 × 2376 pixels (FHD+)",
            "Density: 397 ppi / 480 dpi (XXHDPI)",
            "Font scale: 1.25002",
            "Physical size: 6.6 inches",
            "Aspect ratio: 20:9",
            "Refresh rate: 60.0 Hz, 90.0 Hz, 120.0 Hz",
            "HDR: Supported",
            "HDR capabilities: HDR10, HLG",
            "Wide color gamut: Supported"), "english screen group");
        String unknown = DeviceInfoReport.screen(new ScreenFacts(1080, 2400, 0, 0, 0, 0, null, null, null), Language.CHINESE);
        equal(unknown, String.join("\n",
            "分辨率：1080 × 2400 像素（FHD+）", "密度：未知", "字体缩放：未知", "尺寸大小：未知", "宽高比：20:9",
            "刷新率：未知", "HDR：未知", "HDR 能力：未知", "广色域：未知"), "unknown screen values");
        String noHdr = DeviceInfoReport.screen(new ScreenFacts(720, 1280, 320, 0, 0, 1f, new float[0], new int[0], false), Language.CHINESE);
        check(noHdr.contains("分辨率：720 × 1280 像素（HD）") && noHdr.contains("密度：320 dpi（XHDPI）")
            && noHdr.contains("字体缩放：1.0") && noHdr.contains("宽高比：16:9") && noHdr.contains("HDR：不支持")
            && noHdr.contains("HDR 能力：无") && noHdr.contains("广色域：不支持"), "display without hdr: " + noHdr);
        check(!noHdr.contains("亮度") && !DeviceInfoReport.screen(facts, Language.ENGLISH).contains("Brightness"),
            "brightness is transient and not reported");
        check(DeviceInfoReport.screen(new ScreenFacts(0, 0, 480, 400f, 400f, 1f, null, null, null), Language.CHINESE) == null,
            "no size means no display group");

        equal(DeviceInfoReport.screenResolution(1440, 3200, Language.CHINESE), "1440 × 3200 像素（QHD+）", "qhd plus");
        equal(DeviceInfoReport.screenResolution(1220, 2712, Language.CHINESE), "1220 × 2712 像素（1.5K）", "one point five k");
        equal(DeviceInfoReport.screenResolution(1600, 2560, Language.ENGLISH), "1600 × 2560 pixels", "tablet without tier");
        equal(DeviceInfoReport.screenResolution(1920, 1080, Language.ENGLISH), "1920 × 1080 pixels (FHD)", "landscape sixteen by nine");
        equal(DeviceInfoReport.density(1080, 2400, 440, 0, 0, Language.ENGLISH), "440 dpi (XXHDPI)", "density without physical dpi");
        equal(DeviceInfoReport.density(1080, 2400, 560, 0, 0, Language.ENGLISH), "560 dpi (XXXHDPI)", "xxxhdpi");
        check(DeviceInfoReport.density(1080, 2400, 0, 400f, 400f, Language.ENGLISH) == null, "no density");
        equal(DeviceInfoReport.aspectRatio(1080, 2340), "19.5:9", "nineteen and a half by nine");
        equal(DeviceInfoReport.aspectRatio(1600, 2560), "16:10", "tablet ratio");
        equal(DeviceInfoReport.aspectRatio(1812, 2176), "1.20:1", "foldable inner screen falls back to decimal");
        check(DeviceInfoReport.refreshRates(new float[] {0f, Float.NaN}, Language.CHINESE) == null, "no usable refresh rate");
        equal(DeviceInfoReport.hdrTypes(new int[] {1, 4, 4, 9}, Language.ENGLISH), "Dolby Vision, HDR10+, HDR 9", "hdr names");
        check(DeviceInfoReport.physicalSize(1080, 2400, Float.POSITIVE_INFINITY, 400f, Language.CHINESE) == null, "infinite dpi");
    }
}
