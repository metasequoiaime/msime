package app.msime.android;

/**
 * 键盘底栏：手势导航的手机上，键盘最下一行离屏幕底边只有手势条那一截（常见 16–24 dp，隐藏手势条时是 0），按底行容易碰到系统上滑回桌面的手势区。底栏垫在键区下面，把整块键区抬高 {@link #BAR_HEIGHT_DP}：左边切换输入法，右边剪贴板，中间左右滑动移动光标。
 *
 * <p>只在导航栏是一条细手势条、并且系统没有在那条手势条上自己画输入法按钮时出现。三键导航的导航栏有 48 dp，下面已经有一条够高、带切换按钮的栏。Android 13 起原生系统（Pixel、AOSP 模拟器）在手势导航下，在输入法窗口底部那条 24 dp 的手势条两端画「收起键盘」和「切换输入法」两个按钮；这时导航栏的高度和没有按钮的国产系统一样，只能另外判断，再垫一条底栏按钮会和系统的重复。
 *
 * <p>手机横屏时不出现：横屏的竖向空间本来就紧，键盘高度已经按窗口封顶。浮动键盘悬在应用上面、外接键盘的候选条模式收起了键区，也都不出现。
 */
public final class KeyboardBottomBarPolicy {
    /** 底栏可点的那一截高度。 */
    public static final int BAR_HEIGHT_DP = 40;
    /** 独立留白的取值范围；默认与原来固定留白一致。 */
    public static final int MIN_PADDING_DP = 0;
    public static final int MAX_PADDING_DP = 160;
    public static final int DEFAULT_PADDING_DP = BAR_HEIGHT_DP;
    /** 设置页「键盘底部高度」滑块的步进（#10447）；运行时仍接受 1 dp 粒度的旧值。 */
    public static final int PADDING_STEP_DP = 4;
    /** 导航栏底边低于这个高度才算手势导航的细条；三键导航是 48 dp，手势条常见 16–24 dp。 */
    public static final int THIN_NAVIGATION_MAX_DP = 32;

    private KeyboardBottomBarPolicy() { }

    /**
     * 底栏此刻画不画。
     *
     * @param enabled 本地设置 `platform.android.bottom_bar`
     * @param systemImeButtons 系统自己在手势条上画了输入法的收起和切换按钮
     * @param navigationKnown 能否可靠读到导航栏高度（Android 12 起）；读不到时不画，免得三键导航的设备上叠出第二条栏
     * @param floating 浮动键盘生效中
     * @param keysCollapsed 外接键盘的候选条模式收起了键区
     * @param phoneLandscape 手机（`smallestScreenWidthDp` < 600）横屏
     * @param navigationBottomDp 屏幕底部导航栏的高度（dp）
     */
    public static boolean shown(boolean enabled, boolean systemImeButtons, boolean navigationKnown,
            boolean floating, boolean keysCollapsed, boolean phoneLandscape, float navigationBottomDp) {
        if (!enabled || systemImeButtons || !navigationKnown || floating || keysCollapsed || phoneLandscape)
            return false;
        return Float.isFinite(navigationBottomDp) && navigationBottomDp >= 0
            && navigationBottomDp < THIN_NAVIGATION_MAX_DP;
    }

    /**
     * 「底部留白」此刻垫不垫（#6392）：键区下面垫一段 {@link #BAR_HEIGHT_DP} 高、不可点的空白，把回车、空格这些底行键抬离屏幕底边。
     *
     * <p>不看导航方式：键盘底栏只在细手势条、系统不画输入法按钮时出现，其余设备（系统自己画按钮的 Pixel 一类、Android 11 及以前读不到导航栏高度、三键导航）想抬高底行只能靠它。底栏画着时不再叠加，底栏本身已经把键区抬高了同样的距离。浮动键盘、外接键盘收起键区、手机横屏时同底栏一样不垫。
     *
     * @param enabled 本地设置 `platform.android.bottom_padding`
     * @param barShown 键盘底栏此刻画着（{@link #shown}）
     */
    public static boolean paddingShown(boolean enabled, boolean barShown, boolean floating, boolean keysCollapsed,
            boolean phoneLandscape) {
        return extraPaddingDp(enabled, DEFAULT_PADDING_DP, barShown, floating, keysCollapsed, phoneLandscape) > 0;
    }

    /** 目标留白包含底栏已有的高度；额外空白放在键区与底栏之间，不移动底栏按钮。 */
    public static int extraPaddingDp(boolean enabled, int heightDp, boolean barShown, boolean floating,
            boolean keysCollapsed, boolean phoneLandscape) {
        if (!enabled || floating || keysCollapsed || phoneLandscape) return 0;
        int height = BoundsPolicy.bounded(heightDp, MIN_PADDING_DP, MAX_PADDING_DP);
        return Math.max(0, height - (barShown ? BAR_HEIGHT_DP : 0));
    }

    /**
     * 底栏底边离键盘列底边的距离（像素）：先让出落在键盘窗口里的那截导航栏，系统手势区比导航栏高时再让出高出的部分，这样底栏中间的横向拖动不会从系统的手势区里起手。
     *
     * @param insetBottom 键盘视图收到的底部系统栏 inset，即导航栏落在键盘窗口里的那一截（Android 14 及以前输入法窗口默认停在导航栏上方，这里是 0）
     * @param navigationBottom 屏幕底部导航栏的高度
     * @param gestureBottom 屏幕底部系统手势区的高度
     */
    public static int barBottomPx(int insetBottom, int navigationBottom, int gestureBottom) {
        int inset = BoundsPolicy.nonNegative(insetBottom);
        int navigation = BoundsPolicy.nonNegative(navigationBottom);
        // 某些厂商在 WindowMetrics 报告 0 导航栏时仍把整块底部区域放进 view inset；
        // 这不是底栏应该避让的系统栏，按已知导航栏高度收窄，避免底栏下方留下大块空白。
        inset = Math.min(inset, navigation);
        return inset
            + BoundsPolicy.nonNegative(gestureBottom - navigation);
    }
}
