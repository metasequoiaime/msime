package app.msime.android;

import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.Set;

/**
 * 本包所属的产品版本（edition），由 gradle-app 按版本表 `shared/contracts/editions.json` 为每个 productFlavor 写进 BuildConfig 的 `EDITION`、`EDITION_INPUT_SCHEMES`、`EDITION_DEFAULT_SCHEME` 和 `EDITION_TEMPORARY_JAPANESE`。
 *
 * <p>check-host.sh 用 javac 直接编译输入法服务这部分代码，那里没有 Gradle 生成的 BuildConfig，所以这里经反射读它。没有 BuildConfig（JVM 冒烟测试）就是 full：所有方案、默认全拼，与引入版本之前相同。读到了却不合法时直接失败，一个声明了版本的包不能被当成 full 运行。
 */
public final class AppEdition {
    public static final String FULL_ID = "full";
    /** full：提供全部方案，默认全拼，带临时日语。 */
    public static final AppEdition FULL = new AppEdition(FULL_ID, null, "quanpin", true);
    private static final String BUILD_CONFIG = "app.msime.android.BuildConfig";
    private static volatile AppEdition current;

    private final String id;
    /** 本版本提供的 Engine 方案；null 表示全部（full）。 */
    private final Set<String> inputSchemes;
    private final String defaultScheme;
    private final boolean temporaryJapanese;

    private AppEdition(String id, Set<String> inputSchemes, String defaultScheme,
            boolean temporaryJapanese) {
        this.id = id;
        this.inputSchemes = inputSchemes;
        this.defaultScheme = defaultScheme;
        this.temporaryJapanese = temporaryJapanese;
    }

    /** 由版本 id、逗号分隔的方案列表、默认方案和是否带临时日语组成的版本；默认方案必须在列表里。 */
    public static AppEdition of(String id, String inputSchemes, String defaultScheme,
            boolean temporaryJapanese) {
        if (id == null || id.isEmpty() || inputSchemes == null || defaultScheme == null)
            throw new IllegalArgumentException("Incomplete edition declaration");
        if (FULL_ID.equals(id)) return FULL;
        Set<String> schemes = new LinkedHashSet<>(Arrays.asList(inputSchemes.split(",")));
        schemes.remove("");
        if (!schemes.contains(defaultScheme))
            throw new IllegalArgumentException("Edition default scheme is not one of its schemes");
        return new AppEdition(id, Set.copyOf(schemes), defaultScheme, temporaryJapanese);
    }

    /** 本包的版本，只在第一次调用时读 BuildConfig。 */
    public static AppEdition current() {
        AppEdition value = current;
        if (value != null) return value;
        synchronized (AppEdition.class) {
            if (current == null) current = declared();
            return current;
        }
    }

    private static AppEdition declared() {
        final Class<?> buildConfig;
        try {
            buildConfig = Class.forName(BUILD_CONFIG);
        } catch (ClassNotFoundException absent) {
            return FULL;
        }
        // 两个 Gradle 工程都给每个构建写这几项；有 BuildConfig 却读不到它们，说明字段被裁掉了（例如 R8 没有保留规则），这时宁可失败也不悄悄当成 full。
        try {
            return of((String) buildConfig.getField("EDITION").get(null),
                (String) buildConfig.getField("EDITION_INPUT_SCHEMES").get(null),
                (String) buildConfig.getField("EDITION_DEFAULT_SCHEME").get(null),
                buildConfig.getField("EDITION_TEMPORARY_JAPANESE").getBoolean(null));
        } catch (ReflectiveOperationException incomplete) {
            throw new IllegalStateException("BuildConfig lacks the edition declaration", incomplete);
        }
    }

    public String id() { return id; }
    public boolean isFull() { return inputSchemes == null; }
    /** 本版本的默认方案，也是偏好里的方案本版本没有时的回退值。 */
    public String defaultScheme() { return defaultScheme; }

    /** 本版本是否提供这个 Engine 方案（`quanpin`、`wubi` 等偏好取值）。 */
    public boolean offers(String engineScheme) {
        return inputSchemes == null || inputSchemes.contains(engineScheme);
    }

    /** 本版本是否带临时日语（本地模式 R）。不带时它的日文词典不随包，host-api 也始终把它关掉，键盘不列出这个入口。 */
    public boolean temporaryJapanese() { return temporaryJapanese; }

    /** 本版本是否有不止一个方案可选；只有一个方案时没有「选方案」这回事。 */
    public boolean offersSchemeChoice() {
        return inputSchemes == null || inputSchemes.size() > 1;
    }
}
