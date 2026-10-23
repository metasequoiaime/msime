package app.msime.android;

/**
 * 空格键手势仲裁：长按语音、水平移动光标、上滑复制先满足的一方胜出；没有手势时松手输入空格。
 *
 * <p>纯状态机，不持有计时器：调用方在按下时 {@link #down}，移动时 {@link #move}，计时回调或每次事件时 {@link #tick}，松手时 {@link #up}。
 */
public final class SpaceGesturePolicy {
    /** 长按打开语音的时长。 */
    public static final long LONG_PRESS_MS = 450;
    /** 水平拖动进入光标移动的阈值（dp）。 */
    public static final float DRAG_THRESHOLD_DP = 10f;
    public static final float COPY_THRESHOLD_DP = 24f;

    public enum State { IDLE, PRESSED, VOICE, CURSOR, COPY }

    /** 松手时的结果。 */
    public enum Outcome { SPACE, VOICE, CURSOR, COPY, NONE }

    private State state = State.IDLE;
    private long downAt;
    private float downX;
    private float downY;
    private boolean voiceEnabled = true;
    private boolean copyEnabled;
    private boolean copyArmed;

    public State state() { return state; }

    public void down(long nowMs, float xDp) {
        down(nowMs, xDp, true);
    }

    /** 按下；{@code voiceEnabled} 为 false 时长按不进入语音，停顿后拖动仍可移光标、松手仍是空格。 */
    public void down(long nowMs, float xDp, boolean voiceEnabled) {
        down(nowMs, xDp, 0f, voiceEnabled, false);
    }

    /** 新入口显式提供上滑复制能力，旧调用不改变既有语义。 */
    public void down(long nowMs, float xDp, float yDp, boolean voiceEnabled, boolean copyEnabled) {
        this.voiceEnabled = voiceEnabled;
        this.copyEnabled = copyEnabled;
        copyArmed = false;
        state = State.PRESSED;
        downAt = nowMs;
        downX = xDp;
        downY = yDp;
    }

    /**
     * 手指移动。
     *
     * @return 这次移动是否让空格进入光标移动（只在进入的那一次为 true）
     */
    public boolean move(long nowMs, float xDp) {
        return move(nowMs, xDp, downY);
    }

    public boolean copyArmed() { return state == State.COPY && copyArmed; }

    /** 上滑复制一旦锁定不再变成空格；滑回原处可取消复制。 */
    public boolean move(long nowMs, float xDp, float yDp) {
        float horizontal = Math.abs(xDp - downX);
        float upward = downY - yDp;
        if (state == State.COPY) {
            copyArmed = upward >= COPY_THRESHOLD_DP;
            return false;
        }
        if (state != State.PRESSED) return false;
        if (tick(nowMs)) return false;
        if (copyEnabled && upward >= COPY_THRESHOLD_DP && upward > horizontal * 1.5f) {
            state = State.COPY;
            copyArmed = true;
            return false;
        }
        if (horizontal > DRAG_THRESHOLD_DP && horizontal > Math.abs(yDp - downY)) {
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
        if (!voiceEnabled) return false;
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
            case COPY -> copyArmed ? Outcome.COPY : Outcome.NONE;
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
