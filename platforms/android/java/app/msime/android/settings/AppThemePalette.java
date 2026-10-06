package app.msime.android;

import java.math.BigDecimal;
import java.math.MathContext;
import java.util.Locale;
import org.json.JSONObject;

/**
 * 应用主题（水杉四季 / 春芽 / 夏荫 / 秋杉 / 冬雪）某一季、某一模式下宿主与跟随系统键盘要用的全部颜色，由种子色按设计稿 `全平台 UI.dc.html` 的 `color-mix` 公式推导。
 *
 * <p>种子色（accent、bg、card、hair、onAccent）来自 Rust 的 `msime_client_resolve_app_theme`，季节规则也在那里；这里只做 Android 呈现层的派生：andCard、accentSoft、logo 两色、开屏四色、统计柱与热力图、跟随系统皮肤的键盘底色 / 字母键 / 功能键。宿主（`res/values*` 的主题属性）和 `:ime` 进程（跟随系统皮肤）共用这一份公式，`res` 里的数值与它由测试对照。所有颜色都是 Android 的 ARGB `int`。
 *
 * <p>本类是纯 Java（check-host 直接编译并在 JVM 里跑冒烟），所以不引用 `android.graphics.Color` 也不用任何 androidx 注解；可能为 null 的返回值在 Javadoc 里写明。
 */
public final class AppThemePalette {
    /** Number of packed color channels processed by the palette math (B, G, R, A). */
    public static final int COLOR_CHANNELS = 4;

    // ---- 不随应用主题变化的中性色（设计令牌 §1.1），浅色 / 深色 ----

    public static final int TEXT_LIGHT = 0xFF191C19;
    public static final int TEXT_DARK = 0xFFE1E3DE;
    public static final int SUB_LIGHT = 0xFF414941;
    public static final int SUB_DARK = 0xFFC0C9BF;
    public static final int OUTLINE_LIGHT = 0xFF717970;
    public static final int OUTLINE_DARK = 0xFF8A9389;
    public static final int SWITCH_OFF_LIGHT = 0xFFDFE4DB;
    public static final int SWITCH_OFF_DARK = 0xFF323532;
    /** 深色模式的分隔线；浅色的分隔线随季节，来自种子。 */
    public static final int HAIR_DARK = 0xFF2A2F2A;
    public static final int KEYBOARD_SUB_LIGHT = 0xFF56685A;
    public static final int KEYBOARD_SUB_DARK = 0xFF93A596;
    public static final int KEYBOARD_HAIR_LIGHT = 0x1F000000;
    public static final int KEYBOARD_HAIR_DARK = 0x24FFFFFF;
    public static final int PRESS_LIGHT = 0x12000000;
    public static final int PRESS_DARK = 0x1AFFFFFF;
    public static final int HOVER_LIGHT = 0x0D000000;
    public static final int HOVER_DARK = 0x0FFFFFFF;
    public static final int TOAST_BACKGROUND_LIGHT = 0xFF1C1C1E;
    public static final int TOAST_BACKGROUND_DARK = 0xFFF2F2F2;
    public static final int TOAST_TEXT_LIGHT = 0xFFFFFFFF;
    public static final int TOAST_TEXT_DARK = 0xFF111111;
    /** 「设为默认输入法」尚未完成那一项的橙点，深浅相同。 */
    public static final int WARN = 0xFFFF9500;
    /** 退出登录、注销账号这类破坏性操作的文字，深浅相同。 */
    public static final int DANGER = 0xFFFF3B30;
    /** sheet 与对话框背后的遮罩，深浅相同。 */
    public static final int SCRIM = 0x59000000;

    private static final int WHITE = 0xFFFFFFFF;
    private static final int BLACK = 0xFF000000;
    /** 开屏底色混入的近黑。 */
    private static final int SPLASH_FIELD = 0xFF0A0B0A;
    /** 深色 andCard 混入的底。 */
    private static final int DARK_CARD_BASE = 0xFF1B1D1B;
    private static final int DARK_KEYBOARD_BASE = 0xFF161716;
    private static final int DARK_KEY_BASE = 0xFF3A3C3A;
    private static final int DARK_FUNCTION_BASE = 0xFF262826;
    /** Chrome 打印颜色通道用的有效数字位数，见 {@link #quantize}。 */
    private static final MathContext PRINTED = new MathContext(6);
    /** 热力图五档里 accent 的占比（%）。 */
    private static final int[] HEAT_WEIGHTS = {6, 18, 38, 62, 90};

    public final boolean dark;
    /** colorPrimary：主色。 */
    public final int accent;
    /** colorOnPrimary：主色上的文字与开关开启时的滑块。 */
    public final int onAccent;
    /** colorPrimaryContainer / colorSecondaryContainer：accent 加 `22`（浅）或 `40`（深）透明度。 */
    public final int accentSoft;
    /** android:colorBackground / colorSurface：页面底色（bg）。 */
    public final int background;
    /** colorSurfaceContainer：分组卡、搜索药丸、底部导航（andCard）。 */
    public final int card;
    /** colorSurfaceContainerLowest：详情页普通行的底（rowBg，即种子里的 card）。 */
    public final int rowBackground;
    /** colorOutlineVariant：分隔线（hair）。 */
    public final int hair;
    public final int text;
    public final int sub;
    public final int outline;
    /** colorSurfaceContainerHighest：开关关闭时的轨道。 */
    public final int switchOff;
    /** colorTertiary：logo 标记的填充（logoBg）。 */
    public final int logoBackground;
    /** colorTertiaryContainer：工具栏 logo 的圆盘（logoCirc）。 */
    public final int logoDisc;
    public final int splashBackground;
    public final int splashDisc;
    public final int splashBar;
    /** 开屏光晕中心色：accent 的 34% 透明度，向外渐隐为透明。 */
    public final int splashGlow;
    /** 统计图里非高亮的柱子。 */
    public final int statBar;
    private final int[] heat;
    /** 跟随系统皮肤的键盘面板底色（kb.bg）。 */
    public final int keyboardBackground;
    /** 跟随系统皮肤的字母键（kb.key）。 */
    public final int keyboardKey;
    /** 跟随系统皮肤的功能键（kb.spec）。 */
    public final int keyboardFunction;
    /** 键面文字（kb.fg），与 text 相同。 */
    public final int keyboardText;
    /** 键面角标、空格上的方案名、工具栏图标（kb.sub）。 */
    public final int keyboardSub;
    /** 键盘里的分隔线和未选中的页点（kbHair）。 */
    public final int keyboardHair;
    public final int press;
    public final int hover;
    public final int toastBackground;
    public final int toastText;

    private AppThemePalette(Mode seed, boolean dark) {
        this.dark = dark;
        accent = seed.accent;
        onAccent = seed.onAccent;
        accentSoft = withAlpha(accent, dark ? 0x40 : 0x22);
        background = seed.background;
        rowBackground = seed.card;
        hair = dark ? HAIR_DARK : seed.hair;
        // andCard 不先取整：原型里 logoCirc、统计柱和热力图都是在浏览器未取整的 andCard 上再混一次，先取整会差 1。
        double[] cardExact = dark ? mixExact(channels(accent), 8, channels(DARK_CARD_BASE))
            : mixExact(channels(background), 45, channels(WHITE));
        card = quantize(cardExact);
        text = dark ? TEXT_DARK : TEXT_LIGHT;
        sub = dark ? SUB_DARK : SUB_LIGHT;
        outline = dark ? OUTLINE_DARK : OUTLINE_LIGHT;
        switchOff = dark ? SWITCH_OFF_DARK : SWITCH_OFF_LIGHT;
        logoBackground = mix(accent, 82, BLACK);
        logoDisc = quantize(mixExact(channels(accent), dark ? 22 : 14, cardExact));
        splashBackground = mix(accent, 20, SPLASH_FIELD);
        splashDisc = mix(accent, 12, WHITE);
        splashBar = mix(accent, 70, WHITE);
        splashGlow = withAlpha(accent, 0x57);
        statBar = quantize(mixExact(channels(accent), dark ? 24 : 18, cardExact));
        heat = new int[HEAT_WEIGHTS.length];
        for (int level = 0; level < heat.length; level++) heat[level] = quantize(mixExact(channels(accent), HEAT_WEIGHTS[level], cardExact));
        keyboardBackground = dark ? mix(accent, 10, DARK_KEYBOARD_BASE) : mix(accent, 12, background);
        keyboardKey = dark ? mix(accent, 10, DARK_KEY_BASE) : mix(background, 25, WHITE);
        keyboardFunction = dark ? mix(accent, 14, DARK_FUNCTION_BASE) : mix(accent, 24, background);
        keyboardText = text;
        keyboardSub = dark ? KEYBOARD_SUB_DARK : KEYBOARD_SUB_LIGHT;
        keyboardHair = dark ? KEYBOARD_HAIR_DARK : KEYBOARD_HAIR_LIGHT;
        press = dark ? PRESS_DARK : PRESS_LIGHT;
        hover = dark ? HOVER_DARK : HOVER_LIGHT;
        toastBackground = dark ? TOAST_BACKGROUND_DARK : TOAST_BACKGROUND_LIGHT;
        toastText = dark ? TOAST_TEXT_DARK : TOAST_TEXT_LIGHT;
    }

    /** 这套种子在浅色或深色模式下的全部颜色。 */
    public static AppThemePalette of(Seed seed, boolean dark) {
        return new AppThemePalette(seed.mode(dark), dark);
    }

    /** 热力图第 `level` 档（0–4，越大越接近 accent）。 */
    public int heat(int level) {
        if (level < 0 || level >= heat.length) throw new IllegalArgumentException("Heat level out of range: " + level);
        return heat[level];
    }

    /**
     * CSS `color-mix(in srgb, first weight%, second)`：每个通道（含 alpha）按 `weight` 与 `100 - weight` 线性混合，再按 {@link #quantize} 取整，和原型在浏览器里导出的值一致。
     */
    public static int mix(int first, int weight, int second) {
        return quantize(mixExact(channels(first), weight, channels(second)));
    }

    /** 未取整的混合，通道是 0–255 的浮点数，顺序 B、G、R、A。 */
    private static double[] mixExact(double[] first, int weight, double[] second) {
        if (weight < 0 || weight > 100) throw new IllegalArgumentException("Mix weight out of range: " + weight);
        double[] result = new double[COLOR_CHANNELS];
        for (int index = 0; index < COLOR_CHANNELS; index++) {
            result[index] = (first[index] * weight + second[index] * (100 - weight)) / 100.0;
        }
        return result;
    }

    private static double[] channels(int color) {
        double[] result = new double[COLOR_CHANNELS];
        for (int index = 0; index < COLOR_CHANNELS; index++) result[index] = (color >>> (index * 8)) & 0xFF;
        return result;
    }

    /**
     * 浮点通道取整成 ARGB，取法与设计令牌表的来历一致：那些十六进制值是从 Chrome 的 `getComputedStyle` 读出的 `color(srgb r g b)` 换算来的，Chrome 把 0–1 的通道打印成 6 位有效数字，再乘 255 四舍五入。直接四舍五入会让恰好落在 .5 的通道（如 #FDF9F6 的 249.5、#F5B68F 的 244.5）有的差 1，所以先按 6 位有效数字截一次。
     */
    private static int quantize(double[] channels) {
        int result = 0;
        for (int index = 0; index < COLOR_CHANNELS; index++) {
            double printed = new BigDecimal(channels[index] / 255.0).round(PRINTED).doubleValue();
            long value = Math.round(printed * 255.0);
            result |= (int) BoundsPolicy.bounded(value, 0, 255) << (index * 8);
        }
        return result;
    }

    /** 同一颜色换成指定的 alpha（0–255）。 */
    public static int withAlpha(int color, int alpha) {
        return ColorPolicy.withAlpha(color, alpha);
    }

    /** `#AARRGGBB` 形式，供日志和测试比对。 */
    public static String hex(int color) {
        return String.format(Locale.ROOT, "#%08X", color);
    }

    /**
     * 共享契约里的颜色串（`#RRGGBB`，或 alpha 在后的 `#RRGGBBAA`）转成 ARGB。
     *
     * @return 读不懂时为 null
     */
    public static Integer contractColor(String value) {
        if (value == null || !value.startsWith("#")) return null;
        String digits = value.substring(1);
        if (!digits.matches("[0-9A-Fa-f]{6}|[0-9A-Fa-f]{8}")) return null;
        long parsed = Long.parseLong(digits, 16);
        if (digits.length() == 6) return (int) (0xFF000000L | parsed);
        return (int) (((parsed & 0xFF) << 24) | (parsed >>> 8));
    }

    /**
     * 一个应用主题解析到某一季后的种子色，浅色、深色各一组。
     *
     * <p>`msime_client_resolve_app_theme` 每次只回一个模式，所以完整的种子由两次解析（`dark` 为 false 和 true）组成；只需要当前模式的调用方可以只解析那一个 {@link Mode}。
     */
    public static final class Seed {
        /** 秋杉：没有缓存的季节时宿主与键盘的回退值，也是 `res/values*` 的基础主题。 */
        public static final Seed AUTUMN = new Seed("qiushan", "autumn",
            new Mode(0xFFB5562B, 0xFFF6E9DC, 0xFFFFFBF6, 0x33B5562B, 0xFFFFFFFF),
            new Mode(0xFFF0975F, 0xFF21150F, 0xFF2E1E15, HAIR_DARK, 0xFF3C2618));

        /** 应用主题 id（`siji`、`chunya`、`xiayin`、`qiushan`、`dongxue`）。 */
        public final String id;
        /** 解析到的季节（`spring`、`summer`、`autumn`、`winter`）。 */
        public final String season;
        public final Mode light;
        public final Mode dark;

        public Seed(String id, String season, Mode light, Mode dark) {
            if (id == null || season == null || light == null || dark == null)
                throw new IllegalArgumentException("Incomplete app theme seed");
            this.id = id;
            this.season = season;
            this.light = light;
            this.dark = dark;
        }

        public Mode mode(boolean dark) {
            return dark ? this.dark : light;
        }

        /**
         * 由同一主题、同一月份下 `dark: false` 与 `dark: true` 两次 `resolveAppTheme` 的响应组成种子；两次的 `id` 与 `season` 必须一致。
         *
         * @return 任一响应缺字段、颜色读不懂或两次不一致时为 null
         */
        public static Seed fromResolved(JSONObject light, JSONObject dark) {
            if (light == null || dark == null) return null;
            String id = light.optString("id", "");
            String season = light.optString("season", "");
            if (id.isEmpty() || season.isEmpty()) return null;
            if (!id.equals(dark.optString("id", "")) || !season.equals(dark.optString("season", ""))) return null;
            Mode lightMode = Mode.fromResolved(light);
            Mode darkMode = Mode.fromResolved(dark);
            if (lightMode == null || darkMode == null) return null;
            return new Seed(id, season, lightMode, darkMode);
        }
    }

    /** 一个模式下的五个种子色。 */
    public static final class Mode {
        public final int accent;
        public final int background;
        /** 详情页行底（rowBg）；andCard 由它旁边的 bg 推导，不在种子里。 */
        public final int card;
        public final int hair;
        public final int onAccent;

        public Mode(int accent, int background, int card, int hair, int onAccent) {
            this.accent = accent;
            this.background = background;
            this.card = card;
            this.hair = hair;
            this.onAccent = onAccent;
        }

        /**
         * 一次 `resolveAppTheme` 响应（`{id, season, accent, accent_soft, on_accent, background, card, hair}`）里的种子色；`accent_soft` 由本类自己推导，不读。
         *
         * @return 缺字段或颜色读不懂时为 null
         */
        public static Mode fromResolved(JSONObject response) {
            if (response == null) return null;
            Integer accent = contractColor(response.optString("accent", ""));
            Integer background = contractColor(response.optString("background", ""));
            Integer card = contractColor(response.optString("card", ""));
            Integer hair = contractColor(response.optString("hair", ""));
            Integer onAccent = contractColor(response.optString("on_accent", ""));
            if (accent == null || background == null || card == null || hair == null || onAccent == null) return null;
            return new Mode(accent, background, card, hair, onAccent);
        }
    }
}
