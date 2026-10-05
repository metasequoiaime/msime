package app.msime.android;

/**
 * 空格键的手势判定：按住 450 ms 打开语音输入，水平拖动超过阈值进入光标移动，先满足的一方胜出，之后另一方不再生效；两者都没发生时松手就是普通空格。
 *
 * <p>纯状态机，不持有计时器：调用方在按下时 {@link #down}，移动时 {@link #move}，计时回调或每次事件时 {@link #tick}，松手时 {@link #up}。
 */
public final class SpaceGesturePolicy {
    /** 长按打开语音的时长。 */
    public static final long LONG_PRESS_MS = 450;
    /** 水平拖动进入光标移动的阈值（dp）。 */
    public static final float DRAG_THRESHOLD_DP = 10f;

    public enum State { IDLE, PRESSED, VOICE, CURSOR }

    /** 松手时的结果。 */
    public enum Outcome { SPACE, VOICE, CURSOR, NONE }

    private State state = State.IDLE;
    private long downAt;
    private float downX;

    public State state() { return state; }

    public void down(long nowMs, float xDp) {
        state = State.PRESSED;
        downAt = nowMs;
        downX = xDp;
    }

    /**
     * 手指移动。
     *
     * @return 这次移动是否让空格进入光标移动（只在进入的那一次为 true）
     */
    public boolean move(long nowMs, float xDp) {
        if (state != State.PRESSED) return false;
        if (tick(nowMs)) return false;
        if (Math.abs(xDp - downX) > DRAG_THRESHOLD_DP) {
            state = State.CURSOR;
            return true;
        }
        return false;
    }

    /**
     * 时间推进。
     *
     * @return 这一刻是否刚好进入语音（只在进入的那一次为 true）
     */
    public boolean tick(long nowMs) {
        if (state != State.PRESSED) return false;
        if (nowMs - downAt >= LONG_PRESS_MS) {
            state = State.VOICE;
            return true;
        }
        return false;
    }

    /** 松手，返回这次手势的结果并回到空闲。 */
    public Outcome up(long nowMs) {
        if (state == State.PRESSED) tick(nowMs);
        Outcome outcome = switch (state) {
            case PRESSED -> Outcome.SPACE;
            case VOICE -> Outcome.VOICE;
            case CURSOR -> Outcome.CURSOR;
            case IDLE -> Outcome.NONE;
        };
        state = State.IDLE;
        return outcome;
    }

    /** 手势被系统取消（例如窗口失焦）。 */
    public void cancel() {
        state = State.IDLE;
    }
}
