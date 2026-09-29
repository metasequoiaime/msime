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

    public void toggle(long uptimeMillis) {
        if (uptimeMillis < 0) throw new IllegalArgumentException("Uptime must not be negative");
        automatic = false;
        if (mode == Mode.SHIFTED && lastShiftTapMillis >= 0
                && uptimeMillis >= lastShiftTapMillis
                && uptimeMillis - lastShiftTapMillis <= CAPS_LOCK_INTERVAL_MILLIS) {
            mode = Mode.CAPS_LOCK;
        } else {
            mode = mode == Mode.LOWERCASE ? Mode.SHIFTED : Mode.LOWERCASE;
        }
        lastShiftTapMillis = uptimeMillis;
    }

    public boolean applyAutomatic(boolean shouldShift) {
        if (mode == Mode.CAPS_LOCK) return false;
        Mode previous = mode;
        mode = shouldShift ? Mode.SHIFTED : Mode.LOWERCASE;
        automatic = shouldShift;
        lastShiftTapMillis = -1;
        return previous != mode;
    }

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
