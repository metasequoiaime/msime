package app.msime.android;

/** One-shot Shift, automatic Shift and double-tap Caps Lock state without Android dependencies. */
public final class EnglishLetterCaseState {
    public enum Mode { LOWERCASE, SHIFTED, CAPS_LOCK }

    public static final long CAPS_LOCK_INTERVAL_MILLIS = 350;
    private Mode mode = Mode.LOWERCASE;
    private boolean automatic;
    private long lastShiftTapMillis = -1;

    public Mode mode() { return mode; }
    public boolean usesUppercase() { return mode != Mode.LOWERCASE; }
    public boolean isAutomatic() { return automatic; }

    public void reset() {
        mode = Mode.LOWERCASE;
        automatic = false;
        lastShiftTapMillis = -1;
    }

    /** 双击只看两次手动点按的间隔，不看第一下之后是什么状态：句首自动大写时第一下关掉大写，第二下照样锁定。锁定时点一下回到小写并清掉计时，紧接着的一下不会又锁上。 */
    public void toggle(long uptimeMillis) {
        if (uptimeMillis < 0) throw new IllegalArgumentException("Uptime must not be negative");
        automatic = false;
        if (mode == Mode.CAPS_LOCK) {
            mode = Mode.LOWERCASE;
            lastShiftTapMillis = -1;
            return;
        }
        if (lastShiftTapMillis >= 0
                && uptimeMillis >= lastShiftTapMillis
                && uptimeMillis - lastShiftTapMillis <= CAPS_LOCK_INTERVAL_MILLIS) {
            mode = Mode.CAPS_LOCK;
            lastShiftTapMillis = -1;
            return;
        }
        mode = mode == Mode.LOWERCASE ? Mode.SHIFTED : Mode.LOWERCASE;
        lastShiftTapMillis = uptimeMillis;
    }

    /** 不清点按计时：编辑器在两次点按之间回报一次光标位置（WebView 很常见），不能让双击失效。 */
    public boolean applyAutomatic(boolean shouldShift) {
        if (mode == Mode.CAPS_LOCK) return false;
        Mode previous = mode;
        mode = shouldShift ? Mode.SHIFTED : Mode.LOWERCASE;
        automatic = shouldShift;
        return previous != mode;
    }

    /** 用户手动按下、还没被字母用掉的单次大写。 */
    public boolean isPressedShift() { return mode == Mode.SHIFTED && !automatic; }

    public boolean consumeLetter() {
        if (mode != Mode.SHIFTED) return false;
        mode = Mode.LOWERCASE;
        automatic = false;
        lastShiftTapMillis = -1;
        return true;
    }

    public String keyText() {
        return mode == Mode.CAPS_LOCK ? "⇪" : "⇧";
    }

    public String accessibilityLabel(boolean englishMode) {
        return switch (mode) {
            case LOWERCASE -> englishMode ? "大写" : "切换到英文大写";
            case SHIFTED -> "大写";
            case CAPS_LOCK -> "大写锁定";
        };
    }

    public String accessibilityValue() {
        return switch (mode) {
            case LOWERCASE -> "关闭";
            case SHIFTED -> automatic ? "自动开启" : "下一字母";
            case CAPS_LOCK -> "开启";
        };
    }
}
