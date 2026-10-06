package app.msime.android;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Android rendering values for the touch keyboard under the selected global theme.
 *
 * <p>There is no skin table here any more. The colours come from one of three places: the Material 3 tokens this host draws for `system` (and for any slot a theme leaves null), the keyboard palette `msime_client_resolve_theme` returns for a built-in or custom theme, or the user's full custom design (photo, gradient, key shape and material) when `custom_theme.keyboard` holds one. Every colour is `#RRGGBB` or `#AARRGGBB`, the form `Color.parseColor` reads.
 */
public final class KeyboardSkin {
    /** The design's Android key corner radius, in dp. */
    public static final double KEY_RADIUS_DP = 8;

    private final String id;
    private final String title;
    private final String description;
    private final boolean dark;
    private final boolean designed;
    private final String background;
    private final String keyBackground;
    private final String keyForeground;
    private final String secondary;
    private final String accent;
    private final String onAccent;
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
    private final String hairline;
    private final String toolbarIcon;
    private final String platformAccent;
    private final String platformOnAccent;
    private final String platformAccentSoft;

    /** A flat palette: the M3 tokens or a resolved theme's keyboard. No border, no shadow, radius 8. */
    private KeyboardSkin(String id, String title, String description, boolean dark,
            String background, String keyBackground, String functionBackground, String text,
            String secondary, String accent, String onAccent) {
        this(id, title, description, dark, background, keyBackground, functionBackground, text,
            secondary, accent, onAccent, dark ? HAIRLINE_DARK : HAIRLINE_LIGHT, secondary,
            platformAccent(dark), platformOnAccent(dark), platformAccentSoft(dark));
    }

    /** 扁平配色的完整构造：除了键盘本身的七个槽位，还带分隔线、工具栏图标色，以及回车与开启态瓷砖用的「平台强调色」三件套。 */
    private KeyboardSkin(String id, String title, String description, boolean dark,
            String background, String keyBackground, String functionBackground, String text,
            String secondary, String accent, String onAccent, String hairline, String toolbarIcon,
            String platformAccent, String platformOnAccent, String platformAccentSoft) {
        this.id = id;
        this.title = title;
        this.description = description;
        this.dark = dark;
        designed = false;
        this.background = background;
        this.keyBackground = keyBackground;
        keyForeground = text;
        this.secondary = secondary;
        this.accent = accent;
        this.onAccent = onAccent;
        actionBackground = functionBackground;
        // The design labels its tinted function keys in the key text colour (dc.html X()).
        actionForeground = text;
        cornerRadius = KEY_RADIUS_DP;
        borderWidth = 0;
        borderColor = alpha(accent, .28);
        shadowOpacity = 0;
        shadowRadius = 0;
        shadowOffset = 0;
        monospaced = false;
        pattern = 0;
        keyShape = "rounded";
        keyMaterial = "flat";
        keyOpacity = 1;
        gradientEnd = null;
        gradientHorizontal = false;
        patternOpacity = .15;
        photo = null;
        photoShade = .25;
        photoPosition = .5;
        this.hairline = hairline;
        this.toolbarIcon = toolbarIcon;
        this.platformAccent = platformAccent;
        this.platformOnAccent = platformOnAccent;
        this.platformAccentSoft = platformAccentSoft;
        designKey = String.join(",", background, keyBackground, functionBackground, text,
            secondary, accent, onAccent, hairline, toolbarIcon, platformAccent, platformOnAccent,
            platformAccentSoft);
    }

    private KeyboardSkin(CustomKeyboardSkin design, boolean dark) {
        id = "custom";
        title = "我的皮肤";
        description = "自由配色 · 自定义键帽";
        this.dark = dark;
        designed = true;
        background = design.background();
        keyBackground = design.keyBackground();
        keyForeground = design.keyForeground();
        // The shared flattening (`custom_keyboard`) draws hints in the key text at 0x99. The resolver never sees the design (themeRequest leaves it out), so its `secondary` belongs to the base theme and must not be used here.
        secondary = alpha(design.keyForeground(), 0x99 / 255.0);
        accent = design.accent();
        onAccent = readable(design.accent());
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
        hairline = alpha(design.keyForeground(), 0x1F / 255.0);
        toolbarIcon = secondary;
        platformAccent = accent;
        platformOnAccent = onAccent;
        platformAccentSoft = alpha(accent, 0x24 / 255.0);
    }

    /**
     * Whether one surface draws dark. `surfaceMode` is that surface's own `*_theme` preference and wins when it is `dark` or `light`; anything else follows `appMode` (the `theme` preference), and a `system` app mode follows the Android night mode. A theme with a fixed appearance overrides the result after resolution.
     */
    public static boolean resolveDark(String surfaceMode, String appMode, boolean systemDark) {
        if ("dark".equals(surfaceMode)) return true;
        if ("light".equals(surfaceMode)) return false;
        if ("dark".equals(appMode)) return true;
        if ("light".equals(appMode)) return false;
        return systemDark;
    }

    /**
     * 跟随系统皮肤在没有应用主题时的样子：设计的 classic 基础色（`N/design-tokens.md` §1.4 的 classic 列），按与季节主题相同的公式由 classic 种子推导。
     */
    public static KeyboardSkin system(boolean dark) {
        return system(dark, CLASSIC);
    }

    /**
     * 跟随系统皮肤在某个应用主题（某一季）下的键盘：底色、字母键、功能键按设计令牌 §1.4 的 `color-mix` 公式由种子推导（公式本身在 {@link AppThemePalette}，宿主与 `:ime` 共用），键面文字为 text，角标 / 空格方案名 / 工具栏图标为 kbSub，回车填 accent、字用该季的 onAccent，工具栏激活底为 accentSoft。
     *
     * @param seed 应用主题解析到当前季节后的种子；为 null 时按 classic 基础色
     */
    public static KeyboardSkin system(boolean dark, AppThemePalette.Seed seed) {
        AppThemePalette palette = AppThemePalette.of(seed == null ? CLASSIC : seed, dark);
        String accent = colorString(palette.accent);
        String onAccent = colorString(palette.onAccent);
        return new KeyboardSkin("system", "跟随系统",
            seed == null || seed == CLASSIC ? "Material 3 · 跟随系统明暗" : "跟随应用主题 · 跟随系统明暗",
            dark, colorString(palette.keyboardBackground), colorString(palette.keyboardKey),
            colorString(palette.keyboardFunction), colorString(palette.keyboardText),
            colorString(palette.keyboardSub), accent, onAccent, colorString(palette.keyboardHair),
            colorString(palette.keyboardSub), accent, onAccent, colorString(palette.accentSoft));
    }

    /**
     * 同一套键盘配色，但回车、开启态瓷砖和工具栏激活态改用应用主题当前季节的强调色（accent / onAccent / accentSoft），用于让命名皮肤也跟随应用主题的强调色；用户自己的键盘设计保留它自己的强调色，原样返回。
     *
     * @param seed 应用主题种子；为 null 时原样返回
     */
    public KeyboardSkin withAppTheme(AppThemePalette.Seed seed) {
        if (seed == null || designed) return this;
        AppThemePalette palette = AppThemePalette.of(seed, dark);
        return new KeyboardSkin(id, title, description, dark, background, keyBackground,
            actionBackground, keyForeground, secondary, accent, onAccent, hairline, toolbarIcon,
            colorString(palette.accent), colorString(palette.onAccent),
            colorString(palette.accentSoft));
    }

    /** 设计稿的 classic 应用主题（不可选，只作没有应用主题时的基础色）：浅色 accent #2C7A4B、bg #F7FBF3，深色 accent #8FD5A6、bg #111411、onAccent #003920。 */
    private static final AppThemePalette.Seed CLASSIC = new AppThemePalette.Seed("classic", "classic",
        new AppThemePalette.Mode(0xFF2C7A4B, 0xFFF7FBF3, 0xFFFFFFFF, 0xFFDDE5DB, 0xFFFFFFFF),
        new AppThemePalette.Mode(0xFF8FD5A6, 0xFF111411, 0xFF1B1D1B, AppThemePalette.HAIR_DARK, 0xFF003920));

    /** 键盘里的分隔线与未选中页点（kbHair），浅色 / 深色。 */
    private static final String HAIRLINE_LIGHT = colorString(AppThemePalette.KEYBOARD_HAIR_LIGHT);
    private static final String HAIRLINE_DARK = colorString(AppThemePalette.KEYBOARD_HAIR_DARK);

    /** ARGB 整数转成皮肤里用的颜色串：不透明时 `#RRGGBB`，否则 Android 的 `#AARRGGBB`。 */
    static String colorString(int argb) {
        if ((argb >>> 24) == 0xFF) return String.format(Locale.ROOT, "#%06X", argb & 0xFFFFFF);
        return String.format(Locale.ROOT, "#%08X", argb);
    }

    /**
     * A resolved keyboard palette in the contract's colour form (`#RRGGBB` or `#RRGGBBAA`). A null or unreadable slot is the Material 3 token for that slot, never transparent.
     */
    public static KeyboardSkin palette(String id, String title, boolean dark, String background,
            String key, String functionKey, String text, String secondary, String accent,
            String onAccent) {
        return new KeyboardSkin(id, title, dark ? "深色主题" : "浅色主题", dark,
            slot(background, systemBackground(dark)), slot(key, systemKey(dark)),
            slot(functionKey, systemFunction(dark)), slot(text, systemText(dark)),
            slot(secondary, systemSecondary(dark)), slot(accent, platformAccent(dark)),
            slot(onAccent, platformOnAccent(dark)));
    }

    /** The user's full custom design from `custom_theme.keyboard`. */
    public static KeyboardSkin custom(JSONObject design, boolean dark) {
        return new KeyboardSkin(CustomKeyboardSkin.from(design), dark);
    }

    static KeyboardSkin customFixture(CustomKeyboardSkin design, boolean dark) {
        return new KeyboardSkin(design, dark);
    }

    /**
     * The keyboard for one `msime_client_resolve_theme` value.
     *
     * <p>`hostDark` is the mode the request was made in; a non-null `appearance` fixes the mode instead. `customDesign` is `custom_theme.keyboard` and is drawn in full when the resolved theme is custom, because the flattened palette cannot carry a photo, gradient or key shape. `title` names the theme in the picker and the skin button's description.
     */
    public static KeyboardSkin resolved(JSONObject theme, String title, boolean hostDark,
            JSONObject customDesign) {
        JSONObject keyboard = theme.optJSONObject("keyboard");
        Map<String, String> slots = null;
        if (keyboard != null) {
            slots = new HashMap<>();
            for (String slot : KEYBOARD_SLOTS) slots.put(slot, text(keyboard, slot));
        }
        return resolved(theme.optString("id", "system"), title, text(theme, "appearance"),
            hostDark, slots, customDesign == null ? null : CustomKeyboardSkin.from(customDesign));
    }

    /** The slots of a resolved theme's `keyboard` object. */
    private static final String[] KEYBOARD_SLOTS = {
        "background", "key", "function_key", "text", "secondary", "accent", "on_accent"
    };

    /**
     * {@link #resolved(JSONObject, String, boolean, JSONObject)} on values already read out of the resolver's answer. `appearance` is null when the theme has no fixed mode; `keyboard` maps each of {@link #KEYBOARD_SLOTS} to its colour (a slot may be null) and is itself null when the answer carries no keyboard.
     */
    static KeyboardSkin resolved(String id, String title, String appearance, boolean hostDark,
            Map<String, String> keyboard, CustomKeyboardSkin customDesign) {
        boolean dark = appearance == null ? hostDark : "dark".equals(appearance);
        if ("custom".equals(id) && customDesign != null)
            return new KeyboardSkin(customDesign, dark);
        if (keyboard == null) {
            KeyboardSkin system = system(dark);
            return "system".equals(id) ? system : system.named(id, title);
        }
        return palette(id, title, dark, keyboard.get("background"), keyboard.get("key"),
            keyboard.get("function_key"), keyboard.get("text"), keyboard.get("secondary"),
            keyboard.get("accent"), keyboard.get("on_accent"));
    }

    /**
     * The `msime_client_resolve_theme` request for one global theme in one host mode.
     *
     * <p>Android reads no skin root and no published package catalog, so the request carries neither `skins_directory` nor `package`, and it asks for the horizontal strip. `custom_theme.keyboard` stays out: it can carry a half-megabyte photo, the shared resolver only flattens it to colours, and this host draws the design itself through {@link #resolved}.
     */
    public static String themeRequest(String globalTheme, JSONObject customTheme, boolean dark)
            throws JSONException {
        JSONObject theme = new JSONObject();
        if (customTheme != null) {
            for (java.util.Iterator<String> keys = customTheme.keys(); keys.hasNext(); ) {
                String name = keys.next();
                if (!"keyboard".equals(name)) theme.put(name, customTheme.get(name));
            }
        }
        return new JSONObject()
            .put("global_theme", globalTheme)
            .put("custom_theme", theme)
            .put("dark", dark)
            .put("layout", "horizontal")
            .toString();
    }

    /** The catalog title of one global theme id, or the id itself when the catalog does not list it. */
    public static String themeTitle(JSONArray themes, String id) {
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry != null && id.equals(entry.optString("id")))
                return entry.optString("title", id);
        }
        return id;
    }

    private static String text(JSONObject object, String key) {
        if (object.isNull(key)) return null;
        return object.optString(key, null);
    }

    private KeyboardSkin named(String id, String title) {
        return new KeyboardSkin(id, title, description, dark, background, keyBackground,
            actionBackground, keyForeground, secondary, accent, onAccent, hairline, toolbarIcon,
            platformAccent, platformOnAccent, platformAccentSoft);
    }

    /**
     * `#RRGGBB` stays as it is; `#RRGGBBAA` (alpha last, the shared contract's form) becomes `#AARRGGBB` (alpha first, Android's form). Anything else is null.
     */
    public static String androidColor(String value) {
        if (value == null || !value.startsWith("#")) return null;
        String digits = value.substring(1).toUpperCase(Locale.ROOT);
        if (!digits.matches("[0-9A-F]{6}|[0-9A-F]{8}")) return null;
        return digits.length() == 6 ? "#" + digits
            : "#" + digits.substring(6) + digits.substring(0, 6);
    }

    private static String slot(String value, String fallback) {
        String color = androidColor(value);
        return color == null ? fallback : color;
    }

    // ---- 空槽位的回退：设计 classic 基础键盘色（与 system(boolean) 相同），浅色 / 深色 ----

    private static String systemBackground(boolean dark) { return dark ? "#222A24" : "#DFECDF"; }
    private static String systemKey(boolean dark) { return dark ? "#424B45" : "#FDFEFC"; }
    private static String systemFunction(boolean dark) { return dark ? "#354038" : "#C6DCCB"; }
    private static String systemText(boolean dark) { return dark ? "#E1E3DE" : "#191C19"; }
    private static String systemSecondary(boolean dark) { return dark ? "#93A596" : "#56685A"; }

    /** The platform accent: the return key's fill while composing and every native accent. */
    private static String platformAccent(boolean dark) { return dark ? "#8FD5A6" : "#2C7A4B"; }

    /**
     * Text on the platform accent. The design draws white on both, but white on the dark-mode #8FD5A6 is about 1.9:1, so the dark mode takes Material's on-primary for that green instead.
     */
    private static String platformOnAccent(boolean dark) { return dark ? "#003920" : "#FFFFFF"; }

    /** The tinted surface of a switched-on function tile. */
    private static String platformAccentSoft(boolean dark) { return dark ? "#2A4F37" : "#CFE9D6"; }

    private static String readable(String rgb) {
        int value = Integer.parseInt(rgb.substring(rgb.length() - 6), 16);
        double luminance = .2126 * linear(value >> 16) + .7152 * linear(value >> 8)
            + .0722 * linear(value);
        return luminance > .179 ? "#000000" : "#FFFFFF";
    }

    private static double linear(int channel) {
        double component = (channel & 255) / 255.0;
        return component <= .04045 ? component / 12.92
            : Math.pow((component + .055) / 1.055, 2.4);
    }

    private static int channel(double value) {
        return (int) Math.round(Math.max(0, Math.min(1, value)) * 255);
    }

    /** `color` at a fraction of full opacity; any alpha it already carried is replaced. */
    private static String alpha(String color, double value) {
        return String.format(Locale.ROOT, "#%02X%s", channel(value),
            color.substring(color.length() - 6));
    }

    public String id() { return id; }
    public String title() { return title; }
    public String description() { return description; }
    public boolean dark() { return dark; }
    /** Whether this is the user's full custom design, drawn with its own key drawable and background. */
    public boolean designed() { return designed; }
    public String key() { return id + ":" + dark + (designKey.isEmpty() ? "" : ":" + designKey); }
    public String background() { return background; }
    public String keyBackground() { return keyBackground; }
    public String keyForeground() { return keyForeground; }
    /** Hints, candidate numbers, translations and the space bar's label. */
    public String secondary() { return secondary; }
    /** The selected strip candidate's text and the theme's own accent (drawn with no fill). */
    public String accent() { return accent; }
    /** Text on anything filled with {@link #accent()}. */
    public String onAccent() { return onAccent; }
    /**
     * The function-key face (shift, delete, 123, 中/英, the nine-key side column).
     *
     * A theme tints it with its own function colour. A keyboard design draws it on the letter-key face, as the iOS keyboard does (`KeyboardTheme.functionKeyBackground` returns `keyBackground` for a design): the design's `actionBackground` belongs to the action key alone. Filling every function key with it painted a dozen keys in the action colour and made community skins look like a patchwork.
     */
    public String functionBackground() { return designed ? keyBackground : actionBackground; }
    /** The label on {@link #functionBackground()}: a design's function keys share the letter-key face, so they share its text colour too. */
    public String functionForeground() { return designed ? keyForeground : actionForeground; }
    /**
     * The return key while composing (确认). For a theme it is not a theme colour: the design fills it with the platform accent in every theme (dc.html L2211, THEME_CONTRACT `KeyboardThemePalette`), as the iOS and Harmony keyboards do. A keyboard design fills it with its own `actionBackground`, as iOS does (`SkinKeySurfaceView` draws the action key in `actionBackground`); its `accent` is for borders, patterns and the brand mark, not for a key.
     */
    public String returnBackground() { return designed ? actionBackground : platformAccent; }
    /** The label on {@link #returnBackground()}. */
    public String returnForeground() { return designed ? actionForeground : platformOnAccent; }
    /**
     * A switched-on function tile's surface. The design draws `tileOn` from the platform tokens in every theme (`k.accentSoft`), the same on every host; only the user's own keyboard design tints it with its accent, at the shared selected-candidate tint (0x24).
     */
    public String accentSoft() { return platformAccentSoft; }
    /** A switched-on function tile's label: the platform accent (`k.accentText`), or a keyboard design's own accent. */
    public String accentText() { return platformAccent; }
    public String actionBackground() { return actionBackground; }

    /**
     * The rail the nine-key punctuation column sits on.
     *
     * <p>Those four keys wear no cap of their own, so without a rail behind them the column reads as a hole in the grid. Half the key face is what the shared design puts there.
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
    /** 键盘里的分隔线（候选条与展开键之间的竖线、未选中的页点），即设计的 kbHair。 */
    public String hairline() { return hairline; }
    /** 工具栏图标（表情、常用语、剪贴板、皮肤、输入方式）的颜色，即设计的 kbSub。 */
    public String toolbarIcon() { return toolbarIcon; }
    /**
     * 工具栏按钮激活（对应面板打开）时的圆底。
     *
     * 跟随系统和设计皮肤用 accentSoft。内置的命名皮肤（水杉、浅色、纸白、夜青、墨）的 accentSoft 来自平台的绿色令牌，与皮肤无关；原型在这里漏出一块固定的绿，夜青、墨上尤其扎眼。改为取皮肤自己的强调色，按设计的色调容器比例（浅色 13%、深色 25%）叠底。
     */
    public String toolbarActiveBackground() {
        return namedTheme() ? alpha(accent, dark ? .25 : .13) : accentSoft();
    }
    /** 工具栏按钮激活时的图标色：强调色；命名皮肤用皮肤自己的强调色，理由同上。 */
    public String toolbarActiveIcon() { return namedTheme() ? accent : accentText(); }

    /** 内置的命名皮肤：既不是跟随系统，也不是用户的设计。 */
    private boolean namedTheme() { return !designed && !"system".equals(id); }
    /** 首选候选 chip 的底：字母键的颜色（kb.key）。 */
    public String candidateSelectedBackground() { return keyBackground; }
    /** 首选候选 chip 的字：皮肤的强调色，600 字重。 */
    public String candidateSelectedForeground() { return accent; }
    /** 键面角标、空格上的方案短名（kbSub）；与 {@link #secondary()} 相同。 */
    public String hint() { return secondary; }
}
