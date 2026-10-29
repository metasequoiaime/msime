package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.TreeMap;

/**
 * 「关于」和「帮助与反馈」里的设备信息：每一项怎么显示，以及「复制设备信息」复制出去的文本（#5662）。
 *
 * <p>只有反馈问题时排错要用的基础信息：应用版本、品牌、型号、系统版本、处理器架构、屏幕分辨率、存储空间、运行内存、Android System WebView 版本和键盘是否已启用。没有账号、输入内容、安装的其他应用或任何标识这台设备的 id。读系统的那一半在 {@code DeviceInfoCollector}，这里只做格式化，不依赖 Android。
 *
 * <p>键盘功能菜单的「设备信息」子面板（#10045）另用 {@link Field} 和 {@link Item}：在上面的白名单之外多两项，CPU 名称和一组屏幕参数（分辨率、密度、字体缩放、尺寸、宽高比、刷新率、HDR、HDR 能力、广色域），每项都能按中文或英文标签单独插入。亮度和亮度模式随时在变，不收。
 */
public final class DeviceInfoReport {
    /** 读不到的项显示成这样，而不是空着。 */
    public static final String UNKNOWN = "未知";

    /** 一项：标签和值。 */
    public record Entry(String label, String value) {}

    /** 插入或复制出去的文本用哪种语言的标签和说明；值本身（型号、版本号）不翻译。 */
    public enum Language {
        CHINESE, ENGLISH;

        String unknown() { return this == CHINESE ? UNKNOWN : "Unknown"; }

        /** 「标签：值」里的分隔：中文用全角冒号，英文用半角冒号加空格。 */
        String colon() { return this == CHINESE ? "：" : ": "; }

        /** 列举多个值时的分隔。 */
        String listSeparator() { return this == CHINESE ? "、" : ", "; }

        String parenthesized(String text) { return this == CHINESE ? "（" + text + "）" : " (" + text + ")"; }

        String supported(boolean supported) {
            if (this == CHINESE) return supported ? "支持" : "不支持";
            return supported ? "Supported" : "Not supported";
        }
    }

    /** 键盘子面板里的字段，按全部插入时的顺序排列；屏幕参数是多行的一组，放在最后。 */
    public enum Field {
        APP_VERSION("应用版本", "App version"),
        BRAND("品牌", "Brand"),
        MODEL("型号", "Model"),
        ANDROID("系统版本", "Android version"),
        BUILD("系统构建", "Build"),
        ABIS("处理器架构", "CPU architecture"),
        CPU("CPU 名称", "CPU name"),
        STORAGE("存储空间", "Storage"),
        MEMORY("运行内存", "RAM"),
        WEBVIEW("Android System WebView", "Android System WebView"),
        KEYBOARD("键盘", "Keyboard"),
        DISPLAY("屏幕参数", "Display");

        private final String chinese;
        private final String english;

        Field(String chinese, String english) {
            this.chinese = chinese;
            this.english = english;
        }

        public String label(Language language) { return language == Language.CHINESE ? chinese : english; }
    }

    /** 子面板上能单独插入的九项，按 #10045 列的顺序；应用版本、系统构建和键盘状态只随「全部」一起出去。 */
    public static final List<Field> INSERTABLE = List.of(Field.BRAND, Field.MODEL, Field.ANDROID, Field.ABIS,
        Field.STORAGE, Field.MEMORY, Field.WEBVIEW, Field.CPU, Field.DISPLAY);

    /** 一项读到的值，两种语言各格式化一份，切换语言不用重新读系统；读不到时为 null。 */
    public record Item(Field field, String chinese, String english) {
        public String value(Language language) {
            return valueOrUnknown(language == Language.CHINESE ? chinese : english, language);
        }
    }

    /**
     * 屏幕参数的原始读数，由 {@code DeviceInfoCollector} 从 IME 所在的 Display 读出；读不到的数值为 0，数组和布尔为 null。
     *
     * @param width 当前显示模式的真实像素宽
     * @param height 当前显示模式的真实像素高
     * @param densityDpi 系统用来选资源的逻辑密度
     * @param xdpi 横向物理像素密度，厂商填错或模拟器上可能不准
     * @param ydpi 纵向物理像素密度
     * @param fontScale 系统字体缩放
     * @param refreshRates 显示器支持的各模式刷新率
     * @param hdrTypes `Display.HdrCapabilities` 的 HDR 类型常量
     * @param wideColorGamut 是否广色域
     */
    public record ScreenFacts(int width, int height, int densityDpi, float xdpi, float ydpi, float fontScale,
                              float[] refreshRates, int[] hdrTypes, Boolean wideColorGamut) {}

    private DeviceInfoReport() {}

    /** 复制出去的文本：标题后先说明用途，之后每项一行「标签：值」。 */
    public static String text(String title, List<Entry> entries) {
        StringBuilder text = new StringBuilder(title)
            .append('\n')
            .append("用于反馈问题时定位输入法与设备兼容性；不包含账号、输入内容或设备标识。");
        for (Entry entry : entries) {
            text.append('\n').append(entry.label()).append('：').append(valueOrUnknown(entry.value()));
        }
        return text.toString();
    }

    public static String valueOrUnknown(String value) {
        return valueOrUnknown(value, Language.CHINESE);
    }

    public static String valueOrUnknown(String value, Language language) {
        return value == null || value.isBlank() ? language.unknown() : value.trim();
    }

    /** 键盘子面板复制或「全部」插入的文本：标题后先说明用途，之后按 {@link Field} 的顺序每项一行，屏幕参数自成一组。 */
    public static String text(String title, List<Item> items, Language language) {
        StringBuilder text = new StringBuilder(title).append('\n').append(language == Language.CHINESE
            ? "用于反馈问题时定位输入法与设备兼容性；不包含账号、输入内容或设备标识。"
            : "For locating input method and device compatibility issues in bug reports; contains no account, typed text or device identifier.");
        for (Item item : items) text.append('\n').append(line(item, language));
        return text.toString();
    }

    /** 单独插入的一项：「运行内存：8.0 GB」或「RAM: 8.0 GB」；屏幕参数先写组名，各项另起一行。 */
    public static String line(Item item, Language language) {
        String label = item.field().label(language) + language.colon();
        String value = item.value(language);
        boolean group = item.field() == Field.DISPLAY && !value.equals(language.unknown());
        // 组名后直接换行，英文冒号后的空格不留在行尾。
        return group ? label.stripTrailing() + '\n' + value : label + value;
    }

    /** 从读到的全部字段里取一项；没读到就当读不到。 */
    public static Item item(List<Item> items, Field field) {
        for (Item item : items) if (item.field() == field) return item;
        return new Item(field, null, null);
    }

    /**
     * 连续插入几项时不让它们粘成一行：光标前已有文字且不是换行时，多行输入框先换行，单行输入框先空一格。
     *
     * @param before 光标前的一个字符，读不到或在开头时为 null 或空
     */
    public static String afterText(CharSequence before, boolean multiLine, String text) {
        if (before == null || before.length() == 0) return text;
        char last = before.charAt(before.length() - 1);
        if (last == '\n') return text;
        if (multiLine) return '\n' + text;
        return Character.isWhitespace(last) ? text : ' ' + text;
    }

    /** 「0.2.2（1234）」，带版本时加「 · 五笔版」之类的说明；`code` 为负表示读不到。 */
    public static String appVersion(String name, long code, String edition) {
        return appVersion(name, code, edition, Language.CHINESE);
    }

    public static String appVersion(String name, long code, String edition, Language language) {
        if (name == null || name.isBlank()) return language.unknown();
        StringBuilder text = new StringBuilder(name.trim());
        if (code >= 0) text.append(language.parenthesized(Long.toString(code)));
        if (edition != null && !edition.isBlank()) text.append(" · ").append(edition.trim());
        return text.toString();
    }

    /** 「Android 11（API 30）」。 */
    public static String android(String release, int sdk) {
        return android(release, sdk, Language.CHINESE);
    }

    public static String android(String release, int sdk, Language language) {
        String version = release == null || release.isBlank() ? language.unknown() : release.trim();
        return "Android " + version + language.parenthesized("API " + sdk);
    }

    /** 支持的 ABI，首选的在前：「arm64-v8a、armeabi-v7a」。 */
    public static String abis(String[] abis) {
        return abis(abis, Language.CHINESE);
    }

    public static String abis(String[] abis, Language language) {
        if (abis == null) return language.unknown();
        List<String> names = new ArrayList<>(abis.length);
        for (String abi : abis) if (abi != null && !abi.isBlank()) names.add(abi.trim());
        return names.isEmpty() ? language.unknown() : String.join(language.listSeparator(), names);
    }

    /**
     * 处理器名称，照系统给的原文，不换算成市面上的叫法：Android 12 起是 `Build.SOC_MANUFACTURER` 加 `Build.SOC_MODEL`（如「QTI SM8550」），更早的系统或厂商没填时退回 `Build.HARDWARE`（如「qcom」「mt6785」）。
     *
     * <p>系统读不到时这几项是字面的「unknown」，当作没有。
     */
    public static String cpu(String socManufacturer, String socModel, String hardware) {
        String manufacturer = known(socManufacturer), model = known(socModel);
        if (model != null) return manufacturer == null || model.startsWith(manufacturer) ? model : manufacturer + " " + model;
        return known(hardware);
    }

    private static String known(String value) {
        return value == null || value.isBlank() || value.trim().equalsIgnoreCase("unknown") ? null : value.trim();
    }

    /** 「1080 × 2400 像素 · 440 dpi」；宽高读不到时为「未知」。 */
    public static String resolution(int width, int height, int dpi) {
        if (width <= 0 || height <= 0) return UNKNOWN;
        String size = width + " × " + height + " 像素";
        return dpi > 0 ? size + " · " + dpi + " dpi" : size;
    }

    /** 「可用 23.4 GB / 共 128.0 GB」；总量读不到时为「未知」。 */
    public static String storage(long available, long total) {
        return storage(available, total, Language.CHINESE);
    }

    public static String storage(long available, long total, Language language) {
        if (total <= 0) return language.unknown();
        String free = bytes(BoundsPolicy.bounded(available, 0L, total));
        return language == Language.CHINESE ? "可用 " + free + " / 共 " + bytes(total)
            : free + " free / " + bytes(total) + " total";
    }

    /** 运行内存总量；读不到时为「未知」。 */
    public static String memory(long total, Language language) {
        return total > 0 ? bytes(total) : language.unknown();
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
        return webView(packageName, versionName, Language.CHINESE);
    }

    public static String webView(String packageName, String versionName, Language language) {
        if (packageName == null || packageName.isBlank())
            return language == Language.CHINESE ? "未找到（可能已停用）" : "Not found (may be disabled)";
        String version = versionName == null || versionName.isBlank() ? language.unknown() : versionName.trim();
        return version + language.parenthesized(packageName.trim());
    }

    /** 键盘的状态：「已启用 · 当前默认」「已启用」或「未启用」。 */
    public static String keyboard(boolean enabled, boolean isDefault) {
        return keyboard(enabled, isDefault, Language.CHINESE);
    }

    public static String keyboard(boolean enabled, boolean isDefault, Language language) {
        if (language == Language.CHINESE) {
            if (!enabled) return "未启用";
            return isDefault ? "已启用 · 当前默认" : "已启用";
        }
        if (!enabled) return "Not enabled";
        return isDefault ? "Enabled · default" : "Enabled";
    }

    // ---- 屏幕参数 ----

    /** 屏幕参数一组，每项一行「标签：值」；读不到的项写「未知」，宽高都读不到时整组为 null。 */
    public static String screen(ScreenFacts facts, Language language) {
        if (facts == null || facts.width() <= 0 || facts.height() <= 0) return null;
        boolean chinese = language == Language.CHINESE;
        List<String> lines = new ArrayList<>(9);
        lines.add(screenLine(chinese ? "分辨率" : "Resolution", screenResolution(facts.width(), facts.height(), language), language));
        lines.add(screenLine(chinese ? "密度" : "Density", density(facts.width(), facts.height(), facts.densityDpi(), facts.xdpi(), facts.ydpi(), language), language));
        lines.add(screenLine(chinese ? "字体缩放" : "Font scale", fontScale(facts.fontScale()), language));
        lines.add(screenLine(chinese ? "尺寸大小" : "Physical size", physicalSize(facts.width(), facts.height(), facts.xdpi(), facts.ydpi(), language), language));
        lines.add(screenLine(chinese ? "宽高比" : "Aspect ratio", aspectRatio(facts.width(), facts.height()), language));
        lines.add(screenLine(chinese ? "刷新率" : "Refresh rate", refreshRates(facts.refreshRates(), language), language));
        lines.add(screenLine("HDR", facts.hdrTypes() == null ? null : language.supported(facts.hdrTypes().length > 0), language));
        lines.add(screenLine(chinese ? "HDR 能力" : "HDR capabilities", hdrTypes(facts.hdrTypes(), language), language));
        lines.add(screenLine(chinese ? "广色域" : "Wide color gamut", facts.wideColorGamut() == null ? null : language.supported(facts.wideColorGamut()), language));
        return String.join("\n", lines);
    }

    private static String screenLine(String label, String value, Language language) {
        return label + language.colon() + valueOrUnknown(value, language);
    }

    /** 「1080 × 2400 像素（FHD+）」：档位按短边，比 16:9 更长的屏幕加「+」；不在常见档位上的不标。 */
    public static String screenResolution(int width, int height, Language language) {
        if (width <= 0 || height <= 0) return null;
        String size = width + " × " + height + (language == Language.CHINESE ? " 像素" : " pixels");
        String tier = resolutionTier(width, height);
        return tier == null ? size : size + language.parenthesized(tier);
    }

    /** 常见档位：短边 720 起 HD，1080 起 FHD，1200 起 1.5K，1440 起 QHD，2160 起 UHD；平板常见的 1600、1800 这类短边不归档。 */
    static String resolutionTier(int width, int height) {
        int shorter = Math.min(width, height), longer = Math.max(width, height);
        boolean tall = (long) longer * 9 > (long) shorter * 16;
        if (shorter >= 720 && shorter < 800) return tall ? "HD+" : "HD";
        if (shorter >= 1080 && shorter < 1200) return tall ? "FHD+" : "FHD";
        if (shorter >= 1200 && shorter < 1440) return "1.5K";
        if (shorter >= 1440 && shorter < 1600) return tall ? "QHD+" : "QHD";
        if (shorter >= 2160 && shorter < 2400) return "UHD";
        return null;
    }

    /** 「397 ppi / 480 dpi（XXHDPI）」：ppi 由物理像素密度算出，dpi 是系统选资源用的逻辑密度；物理密度读不到时只写 dpi。 */
    public static String density(int width, int height, int densityDpi, float xdpi, float ydpi, Language language) {
        if (densityDpi <= 0) return null;
        String dpi = densityDpi + " dpi" + language.parenthesized(densityBucket(densityDpi));
        double inches = diagonalInches(width, height, xdpi, ydpi);
        if (inches <= 0) return dpi;
        long ppi = Math.round(Math.hypot(width, height) / inches);
        return ppi + " ppi / " + dpi;
    }

    /** 系统资源的密度档：不超过某档上限就归那一档，与系统缩小高一档资源的做法一致。 */
    static String densityBucket(int densityDpi) {
        if (densityDpi <= 120) return "LDPI";
        if (densityDpi <= 160) return "MDPI";
        if (densityDpi <= 213) return "TVDPI";
        if (densityDpi <= 240) return "HDPI";
        if (densityDpi <= 320) return "XHDPI";
        if (densityDpi <= 480) return "XXHDPI";
        return "XXXHDPI";
    }

    /** 系统给的原值，如「1.0」「1.25002」。 */
    public static String fontScale(float scale) {
        return scale > 0 && Float.isFinite(scale) ? Float.toString(scale) : null;
    }

    /** 对角线英寸数，保留一位：「6.6 英寸」；物理密度读不到时为 null。 */
    public static String physicalSize(int width, int height, float xdpi, float ydpi, Language language) {
        double inches = diagonalInches(width, height, xdpi, ydpi);
        if (inches <= 0) return null;
        return String.format(Locale.ROOT, "%.1f", inches) + (language == Language.CHINESE ? " 英寸" : " inches");
    }

    private static double diagonalInches(int width, int height, float xdpi, float ydpi) {
        if (width <= 0 || height <= 0 || !(xdpi > 0) || !(ydpi > 0) || !Float.isFinite(xdpi) || !Float.isFinite(ydpi)) return 0;
        return Math.hypot(width / (double) xdpi, height / (double) ydpi);
    }

    /** 常见比例的名字，长边比短边，按 N:9 或惯用写法。 */
    private static final String[] RATIO_NAMES = {"1:1", "4:3", "3:2", "16:10", "5:3", "16:9", "18:9", "18.5:9", "19:9", "19.5:9", "20:9", "21:9"};
    private static final double[] RATIO_VALUES = {1, 4 / 3d, 1.5, 1.6, 5 / 3d, 16 / 9d, 2, 18.5 / 9, 19 / 9d, 19.5 / 9, 20 / 9d, 21 / 9d};

    /** 「20:9」：长边比短边，离最近的常见比例在 1.5% 以内就用它的名字（2376 × 1080 是 20:9），否则写成「2.35:1」。 */
    public static String aspectRatio(int width, int height) {
        if (width <= 0 || height <= 0) return null;
        double ratio = Math.max(width, height) / (double) Math.min(width, height);
        int nearest = 0;
        for (int index = 1; index < RATIO_VALUES.length; index++)
            if (Math.abs(RATIO_VALUES[index] - ratio) < Math.abs(RATIO_VALUES[nearest] - ratio)) nearest = index;
        if (Math.abs(RATIO_VALUES[nearest] - ratio) / RATIO_VALUES[nearest] <= 0.015) return RATIO_NAMES[nearest];
        return String.format(Locale.ROOT, "%.2f:1", ratio);
    }

    /** 支持的刷新率从低到高去重：「60.0 Hz、120.0 Hz」。 */
    public static String refreshRates(float[] rates, Language language) {
        if (rates == null) return null;
        TreeMap<Float, String> distinct = new TreeMap<>();
        for (float rate : rates) {
            if (!(rate > 0) || !Float.isFinite(rate)) continue;
            String text = String.format(Locale.ROOT, "%.1f Hz", rate);
            if (!distinct.containsValue(text)) distinct.put(rate, text);
        }
        return distinct.isEmpty() ? null : String.join(language.listSeparator(), distinct.values());
    }

    /** HDR 格式：「HDR10、HLG」；屏幕不支持任何 HDR 时为「无」。类型值是 `Display.HdrCapabilities` 的常量。 */
    public static String hdrTypes(int[] types, Language language) {
        if (types == null) return null;
        if (types.length == 0) return language == Language.CHINESE ? "无" : "None";
        List<String> names = new ArrayList<>(types.length);
        for (int type : types) {
            String name = switch (type) {
                case 1 -> "Dolby Vision";
                case 2 -> "HDR10";
                case 3 -> "HLG";
                case 4 -> "HDR10+";
                default -> "HDR " + type;
            };
            if (!names.contains(name)) names.add(name);
        }
        return String.join(language.listSeparator(), names);
    }
}
