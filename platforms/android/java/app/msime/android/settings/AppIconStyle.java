package app.msime.android;

import java.util.List;

/** 主屏幕图标的配色：每种都对应一个 launcher 别名，真实宿主 Activity 不参与切换。 */
public enum AppIconStyle {
    CLASSIC("classic", "MainActivityClassic", "经典", "深绿描线的水杉"),
    FOREST("forest", "MainActivityForest", "林绿", "饱和度更高的森林绿"),
    SKY("sky", "MainActivitySky", "天青", "浅蓝底色"),
    DUSK("dusk", "MainActivityDusk", "暮色", "暗紫与橘的渐层"),
    VERMILION("vermilion", "MainActivityVermilion", "朱砂", "暖红底色");

    private final String id;
    private final String alias;
    private final String title;
    private final String description;

    AppIconStyle(String id, String alias, String title, String description) {
        this.id = id;
        this.alias = alias;
        this.title = title;
        this.description = description;
    }

    public String id() { return id; }

    /** 清单中的别名类名。 */
    public String alias() { return alias; }

    public String title() { return title; }

    public String description() { return description; }

    public static List<AppIconStyle> all() { return List.of(values()); }

    /** 按已保存的 id 选择配色；未知值回退到清单默认启用的经典别名。 */
    public static AppIconStyle from(String value) {
        for (AppIconStyle style : values()) {
            if (style.id.equals(value)) return style;
        }
        return CLASSIC;
    }

    /** 别名使用命名空间，不能用带版本后缀的 applicationId 拼类名。 */
    public String component() {
        return namespace() + "." + alias;
    }

    /** 清单相对组件名使用的命名空间。 */
    public static String namespace() {
        return AppIconStyle.class.getPackage().getName();
    }
}
