package app.msime.android;

/**
 * 键盘底栏：手势导航的手机上，键盘最下一行离屏幕底边只有手势条那一截（常见 16–24 dp，隐藏手势条时是 0），按底行容易碰到系统上滑回桌面的手势区。底栏垫在键区下面，把整块键区抬高 {@link #BAR_HEIGHT_DP}：左边切换输入法，右边剪贴板，中间左右滑动移动光标。
 *
 * <p>只在导航栏是一条细手势条时出现。三键导航的导航栏有 48 dp；Pixel 等原生系统在手势导航下自己在输入法窗口底部画「收起键盘」和「切换输入法」两个按钮，给输入法窗口的导航栏高度同样是 48 dp 的框高。这两种情况下面已经有一条够高、带切换按钮的栏，再垫一条只会浪费高度，按钮还和系统的重复。
 *
 * <p>手机横屏时不出现：横屏的竖向空间本来就紧，键盘高度已经按窗口封顶。浮动键盘悬在应用上面、外接键盘的候选条模式收起了键区，也都不出现。
 */
public final class KeyboardBottomBarPolicy {
    /** 底栏可点的那一截高度。 */
    public static final int BAR_HEIGHT_DP = 40;
    /** 导航栏底边低于这个高度才算手势导航的细条；三键导航和系统自己画输入法导航按钮时都是 48 dp。 */
    public static final int THIN_NAVIGATION_MAX_DP = 32;

    private KeyboardBottomBarPolicy() { }

    /**
     * 底栏此刻画不画。
     *
     * @param enabled 本地设置 `platform.android.bottom_bar`
     * @param navigationKnown 能否可靠读到导航栏高度（Android 12 起）；读不到时不画，免得三键导航的设备上叠出第二条栏
     * @param floating 浮动键盘生效中
     * @param keysCollapsed 外接键盘的候选条模式收起了键区
     * @param phoneLandscape 手机（`smallestScreenWidthDp` < 600）横屏
     * @param navigationBottomDp 屏幕底部导航栏的高度（dp）
     */
    public static boolean shown(boolean enabled, boolean navigationKnown, boolean floating,
            boolean keysCollapsed, boolean phoneLandscape, float navigationBottomDp) {
        if (!enabled || !navigationKnown || floating || keysCollapsed || phoneLandscape) return false;
        return Float.isFinite(navigationBottomDp) && navigationBottomDp >= 0
            && navigationBottomDp < THIN_NAVIGATION_MAX_DP;
    }

    /**
     * 底栏底边离键盘列底边的距离（像素）：先让出落在键盘窗口里的那截导航栏，系统手势区比导航栏高时再让出高出的部分，这样底栏中间的横向拖动不会从系统的手势区里起手。
     *
     * @param insetBottom 键盘视图收到的底部系统栏 inset，即导航栏落在键盘窗口里的那一截（Android 14 及以前输入法窗口默认停在导航栏上方，这里是 0）
     * @param navigationBottom 屏幕底部导航栏的高度
     * @param gestureBottom 屏幕底部系统手势区的高度
     */
    public static int barBottomPx(int insetBottom, int navigationBottom, int gestureBottom) {
        return Math.max(0, insetBottom) + Math.max(0, gestureBottom - Math.max(0, navigationBottom));
    }
}
