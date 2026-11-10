package app.msime.android;

/** 按 Android 窗口形态选择键盘高度本机设置的规则。 */
public final class KeyboardHeightSettingPolicy {
    private KeyboardHeightSettingPolicy() {}

    /** 当前形态保存键盘高度所用的本机设置键。 */
    public static String keyFor(int smallestScreenWidthDp) {
        return KeyboardFormFactorPolicy.expanded(smallestScreenWidthDp)
            ? AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT_LARGE
            : AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT;
    }

    /**
     * 读取当前形态的键盘高度。
     *
     * <p>展开屏没有单独保存过时回退到旧键，保证升级后第一次切到内屏仍沿用原来的高度；窄屏始终只读旧键。
     */
    public static int adjustment(AndroidLocalSettings.Snapshot local, int smallestScreenWidthDp,
                                 int sharedAdjustment) {
        String key = keyFor(smallestScreenWidthDp);
        if (local.has(key)) return KeyboardHeightPolicy.designHeightAdjustment(local.integer(key));
        if (KeyboardFormFactorPolicy.expanded(smallestScreenWidthDp)
                && local.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)) {
            return KeyboardHeightPolicy.designHeightAdjustment(
                local.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT));
        }
        return KeyboardHeightPolicy.designHeightAdjustment(sharedAdjustment);
    }
}
