package app.msime.client;

import java.util.List;
import java.util.Locale;
import org.json.JSONObject;

/** Android rendering values for Apple's independent touch-keyboard skin preference. */
public final class KeyboardSkin {
    private final String id;
    private final String title;
    private final String description;
    private final boolean dark;
    private final String background;
    private final String keyBackground;
    private final String keyForeground;
    private final String accent;
    private final String actionBackground;
    private final String actionForeground;
    private final double cornerRadius;
    private final double borderWidth;
    private final String borderColor;
    private final double shadowOpacity;
    private final double shadowRadius;
    private final double shadowOffset;
    private final boolean monospaced;
    private final int pattern;
    private final String keyShape;
    private final String keyMaterial;
    private final double keyOpacity;
    private final String gradientEnd;
    private final boolean gradientHorizontal;
    private final double patternOpacity;
    private final byte[] photo;
    private final double photoShade;
    private final double photoPosition;
    private final String designKey;

    private KeyboardSkin(String id, String title, String description, boolean dark,
            String background, String keyBackground, String keyForeground, String accent,
            String actionBackground, double cornerRadius, double borderWidth,
            double shadowOpacity, double shadowRadius, double shadowOffset,
            boolean monospaced, int pattern) {
        this.id = id;
        this.title = title;
        this.description = description;
        this.dark = dark;
        this.background = background;
        this.keyBackground = keyBackground;
        this.keyForeground = keyForeground;
        this.accent = accent;
        this.actionBackground = actionBackground;
        this.actionForeground = "#FFFFFF";
        this.cornerRadius = cornerRadius;
        this.borderWidth = borderWidth;
        this.borderColor = alpha(accent, "midnight".equals(id) ? 0.65 : 0.28);
        this.shadowOpacity = shadowOpacity;
        this.shadowRadius = shadowRadius;
        this.shadowOffset = shadowOffset;
        this.monospaced = monospaced;
        this.pattern = pattern;
        keyShape = "rounded";
        keyMaterial = "flat";
        keyOpacity = 1;
        gradientEnd = null;
        gradientHorizontal = false;
        patternOpacity = .15;
        photo = null;
        photoShade = .25;
        photoPosition = .5;
        designKey = "";
    }

    private KeyboardSkin(CustomKeyboardSkin design, boolean dark) {
        id = "custom";
        title = "我的皮肤";
        description = "自由配色 · 自定义键帽";
        this.dark = dark;
        background = design.background();
        keyBackground = design.keyBackground();
        keyForeground = design.keyForeground();
        accent = design.accent();
        actionBackground = design.actionBackground();
        actionForeground = design.actionForeground();
        cornerRadius = design.cornerRadius();
        borderWidth = design.borderWidth();
        borderColor = design.borderColor();
        shadowOpacity = design.shadow();
        shadowRadius = 2;
        shadowOffset = 1;
        monospaced = design.monospaced();
        pattern = design.pattern();
        keyShape = design.keyShape();
        keyMaterial = design.keyMaterial();
        keyOpacity = design.keyOpacity();
        gradientEnd = design.gradientEnd();
        gradientHorizontal = design.gradientHorizontal();
        patternOpacity = design.patternOpacity();
        photo = design.photo();
        photoShade = design.photoShade();
        photoPosition = design.photoPosition();
        designKey = design.key();
    }

    public static KeyboardSkin from(String value) { return from(value, false); }

    public static boolean resolveDark(String keyboardTheme, String globalTheme, boolean systemDark) {
        if ("dark".equals(keyboardTheme)) return true;
        if ("light".equals(keyboardTheme)) return false;
        if ("dark".equals(globalTheme)) return true;
        if ("light".equals(globalTheme)) return false;
        return systemDark;
    }

    public static KeyboardSkin from(String value, boolean dark) { return from(value, dark, null); }

    public static KeyboardSkin from(String value, boolean dark, JSONObject customDesign) {
        String id = value == null ? "" : value;
        return switch (id) {
            case "custom" -> new KeyboardSkin(CustomKeyboardSkin.from(customDesign), dark);
            case "ocean" -> skin(id, "海盐蓝", "海盐浅蓝 · 轻盈平面", dark,
                adaptive(dark, rgb(.90, .94, .98), rgb(.09, .12, .17)),
                adaptive(dark, "#FFFFFF", rgb(.18, .22, .29)),
                label(dark), adaptive(dark, rgb(.12, .36, .64), rgb(.50, .74, .98)),
                adaptive(dark, rgb(.12, .36, .64), rgb(.16, .36, .62)), 8, 0, 0, 3, 2,
                false, 0);
            case "rose" -> skin(id, "浅蔷薇", "柔和蔷薇 · 简洁圆角", dark,
                adaptive(dark, rgb(.98, .91, .94), rgb(.16, .10, .13)),
                adaptive(dark, "#FFFFFF", rgb(.27, .19, .23)),
                label(dark), adaptive(dark, rgb(.63, .25, .39), rgb(.96, .62, .74)),
                adaptive(dark, rgb(.63, .25, .39), rgb(.56, .23, .36)), 8, 0, 0, 3, 2,
                false, 0);
            case "porcelain" -> skin(id, "素白瓷", "细线边框 · 克制直角", dark,
                adaptive(dark, rgb(.92, .93, .94), rgb(.10, .11, .13)),
                adaptive(dark, rgb(.99, .99, .99), rgb(.20, .21, .23)),
                label(dark), adaptive(dark, rgb(.20, .24, .28), rgb(.80, .84, .89)),
                adaptive(dark, rgb(.20, .24, .28), rgb(.27, .31, .36)), 3, .5, 0, 3, 2,
                false, 0);
            case "typewriter" -> skin(id, "纸上时光", "暖纸网点 · 复古键帽", dark,
                adaptive(dark, rgb(.89, .84, .74), rgb(.15, .13, .10)),
                adaptive(dark, rgb(.99, .96, .88), rgb(.25, .22, .17)),
                label(dark), adaptive(dark, rgb(.37, .25, .15), rgb(.87, .72, .51)),
                adaptive(dark, rgb(.37, .25, .15), rgb(.40, .28, .18)), 5, 1, .30, 0, 3,
                true, 1);
            case "candy" -> skin(id, "奶油桃桃", "奶油波纹 · 饱满圆角", dark,
                adaptive(dark, rgb(.99, .88, .82), rgb(.19, .12, .15)),
                adaptive(dark, rgb(1, .97, .93), rgb(.30, .20, .24)),
                label(dark), adaptive(dark, rgb(.58, .22, .32), rgb(1, .66, .73)),
                adaptive(dark, rgb(.58, .22, .32), rgb(.58, .22, .32)), 18, 0, .16, 3, 2,
                false, 3);
            case "midnight" -> skin(id, "霓虹夜航", "紫色星点 · 霓虹描边", dark,
                rgb(.075, .06, .14), rgb(.16, .12, .25), "#FFFFFF", rgb(.78, .69, 1),
                rgb(.40, .23, .70), 10, 1, 0, 3, 2, false, 1);
            case "blueprint" -> skin(id, "工程蓝图", "蓝图网格 · 等宽字形", dark,
                rgb(.055, .13, .22), rgb(.09, .20, .32), "#FFFFFF", rgb(.54, .84, 1),
                rgb(.12, .34, .54), 3, 1, 0, 3, 2, true, 2);
            default -> skin("forest", "水杉绿", "清新留白 · 经典圆角", dark,
                adaptive(dark, rgb(.91, .94, .92), rgb(.09, .13, .11)),
                adaptive(dark, "#FFFFFF", rgb(.19, .24, .21)),
                label(dark), adaptive(dark, rgb(.094, .36, .28), rgb(.45, .80, .65)),
                adaptive(dark, rgb(.094, .36, .28), rgb(.12, .38, .29)), 8, 0, 0, 3, 2,
                false, 0);
        };
    }

    private static KeyboardSkin skin(String id, String title, String description, boolean dark,
            String background, String keyBackground, String keyForeground, String accent,
            String actionBackground, double cornerRadius, double borderWidth,
            double shadowOpacity, double shadowRadius, double shadowOffset,
            boolean monospaced, int pattern) {
        return new KeyboardSkin(id, title, description, dark, background, keyBackground,
            keyForeground, accent, actionBackground, cornerRadius, borderWidth, shadowOpacity,
            shadowRadius, shadowOffset, monospaced, pattern);
    }

    private static String adaptive(boolean dark, String light, String darkValue) {
        return dark ? darkValue : light;
    }

    private static String label(boolean dark) { return dark ? "#FFFFFF" : "#000000"; }

    private static String rgb(double red, double green, double blue) {
        return String.format(Locale.ROOT, "#%02X%02X%02X", channel(red), channel(green), channel(blue));
    }

    private static int channel(double value) {
        return (int) Math.round(KeyboardGeometry.bounded(value, 0, 1) * 255);
    }

    private static String alpha(String rgb, double value) {
        return String.format(Locale.ROOT, "#%02X%s", channel(value), rgb.substring(1));
    }

    public static List<KeyboardSkin> builtIns(boolean dark) {
        return List.of(from("forest", dark), from("ocean", dark), from("rose", dark),
            from("porcelain", dark), from("typewriter", dark), from("candy", dark),
            from("midnight", dark), from("blueprint", dark));
    }

    public static List<KeyboardSkin> choices(boolean dark, JSONObject customDesign) {
        return List.of(from("forest", dark), from("ocean", dark), from("rose", dark),
            from("porcelain", dark), from("typewriter", dark), from("candy", dark),
            from("midnight", dark), from("blueprint", dark), from("custom", dark, customDesign));
    }

    static KeyboardSkin customFixture(CustomKeyboardSkin design, boolean dark) {
        return new KeyboardSkin(design, dark);
    }

    public String id() { return id; }
    public String title() { return title; }
    public String description() { return description; }
    public boolean dark() { return dark; }
    public String key() { return id + ":" + dark + (designKey.isEmpty() ? "" : ":" + designKey); }
    public String background() { return background; }
    public String keyBackground() { return keyBackground; }
    public String keyForeground() { return keyForeground; }
    public String accent() { return accent; }
    public String actionBackground() { return actionBackground; }

    /**
     * The rail the nine-key punctuation column sits on.
     *
     * <p>Those four keys wear no cap of their own, so without a rail behind them the column reads as
     * a hole in the grid. Half the key face is what the shared design puts there.
     */
    public String sidebarBackground() { return alpha(keyBackground, .5); }
    public String actionForeground() { return actionForeground; }
    public double cornerRadius() { return cornerRadius; }
    public double borderWidth() { return borderWidth; }
    public String borderColor() { return borderColor; }
    public double shadowOpacity() { return shadowOpacity; }
    public double shadowRadius() { return shadowRadius; }
    public double shadowOffset() { return shadowOffset; }
    public boolean monospaced() { return monospaced; }
    public int pattern() { return pattern; }
    public String keyShape() { return keyShape; }
    public String keyMaterial() { return keyMaterial; }
    public double keyOpacity() { return keyOpacity; }
    public String gradientEnd() { return gradientEnd; }
    public boolean gradientHorizontal() { return gradientHorizontal; }
    public double patternOpacity() { return patternOpacity; }
    public byte[] photo() { return photo == null ? null : photo.clone(); }
    public double photoShade() { return photoShade; }
    public double photoPosition() { return photoPosition; }
}
