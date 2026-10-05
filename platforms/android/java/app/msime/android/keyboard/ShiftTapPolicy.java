package app.msime.android;

/**
 * Shift 键的点按状态：关闭时单击进入一次性大写，输入一个字母后自动回到关闭；350 ms 内连点两下进入大写锁定；锁定或一次性状态下再单击回到关闭。
 */
public final class ShiftTapPolicy {
    /** 双击锁定的时间窗口。 */
    public static final long DOUBLE_TAP_MS = 350;

    public enum State { OFF, ONCE, LOCKED }

    private State state = State.OFF;
    private long lastTapAt = Long.MIN_VALUE;

    public State state() { return state; }

    public boolean uppercase() { return state != State.OFF; }

    public boolean locked() { return state == State.LOCKED; }

    /** 点一次 Shift，返回之后的状态。 */
    public State tap(long nowMs) {
        boolean doubleTap = lastTapAt != Long.MIN_VALUE && nowMs - lastTapAt <= DOUBLE_TAP_MS;
        if (doubleTap && state == State.ONCE) {
            state = State.LOCKED;
            lastTapAt = Long.MIN_VALUE;
            return state;
        }
        state = state == State.OFF ? State.ONCE : State.OFF;
        lastTapAt = state == State.ONCE ? nowMs : Long.MIN_VALUE;
        return state;
    }

    /** 输入了一个字母：一次性大写用完回到关闭，锁定保持。 */
    public void letterTyped() {
        if (state == State.ONCE) state = State.OFF;
        lastTapAt = Long.MIN_VALUE;
    }

    /** 切换语言、层或输入框时复位。 */
    public void reset() {
        state = State.OFF;
        lastTapAt = Long.MIN_VALUE;
    }
}
