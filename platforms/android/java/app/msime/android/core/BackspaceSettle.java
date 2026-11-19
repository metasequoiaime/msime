package app.msime.android;

/**
 * 交给编辑器的退格（`KEYCODE_DEL` 按键事件）从发出到编辑器确认之间的状态，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>按键事件和 `InputConnection` 的写入走两条通道，到了应用那边不保证先后：先发退格再上屏，退格可能落在新文字之后把它删掉；先上屏再发退格，退格可能落在上屏之前。模拟器上普通 `EditText` 里「上屏、退格、上屏」200 次全部错位；真机 QQ 里退格后约 40 ms 内开始组字，10 次里 8 次新组字被退格删掉。所以：
 *
 * <ul>
 *   <li>输入法自己的写入还有回声没到时，退格先等（{@link #request}），等到回声或超时再发；</li>
 *   <li>发出之后，在编辑器回报选区（{@link #echo}）或超时之前，键盘不处理新的按键（{@link #holding}），连删的下一次也等着。</li>
 * </ul>
 *
 * <p>空输入框里的退格（引用块由应用处理）没有选区变化，编辑器不回报，只能等超时。超时之后照常继续：编辑器不回报的情况退回以前的行为，不会卡住键盘。
 */
public final class BackspaceSettle {
    /** 等编辑器确认的上限。真机 QQ 回报在 17–51 ms，微信 3–22 ms，模拟器 2–4 ms。 */
    public static final long TIMEOUT_MS = 100;

    private enum State { IDLE, WAITING, SENT }

    private State state = State.IDLE;
    /** WAITING：开始等先前写入回声的时刻；SENT：发出退格的时刻。 */
    private long since;
    /** 最近一次看到的输入法写入预期总数（{@link SelectionEchoTracker#expectations()}）和第一次看到它的时刻。 */
    private long seenExpectations = -1;
    private long seenAt;

    /**
     * 要发一次退格。`ownEchoPending` 是输入法自己的写入还有没对上的回声，`expectations` 是到现在为止记下过的预期总数。
     *
     * <p>可以立刻发返回 true；返回 false 时进入等待，调用方在回声到达（{@link #echo}）或 {@link #deadline()} 时再问 {@link #ready}。已经有一次退格在路上（发出未确认，或在等）时也返回 false，连删排在它后面。
     */
    public boolean request(long now, boolean ownEchoPending, long expectations) {
        if (state == State.SENT && now - since < TIMEOUT_MS) return false;
        if (expectations != seenExpectations) {
            seenExpectations = expectations;
            seenAt = now;
        }
        if (ownEchoPending && now - seenAt < TIMEOUT_MS) {
            if (state != State.WAITING) {
                state = State.WAITING;
                since = now;
            }
            return false;
        }
        state = State.IDLE;
        return true;
    }

    /** 等着的退格现在能不能发：先前写入的回声都到了，或者已经等满 {@link #TIMEOUT_MS}。 */
    public boolean ready(long now, boolean ownEchoPending) {
        if (state != State.WAITING) return false;
        if (ownEchoPending && now - seenAt < TIMEOUT_MS) return false;
        state = State.IDLE;
        return true;
    }

    /** 退格已经发出：在回声或超时之前算未确认。 */
    public void sent(long now) {
        state = State.SENT;
        since = now;
    }

    /** 编辑器回报了选区。发出的退格因此确认时返回 true（调用方放开按住的触摸、发连删的下一次）。 */
    public boolean echo() {
        if (state != State.SENT) return false;
        state = State.IDLE;
        return true;
    }

    /** 到时间了：未确认的退格按超时放行时返回 true。 */
    public boolean expire(long now) {
        if (state != State.SENT || now - since < TIMEOUT_MS) return false;
        state = State.IDLE;
        return true;
    }

    /** 有退格在路上（在等或发出未确认）：新开始的触摸先按住，连删的下一次也排队。 */
    public boolean holding(long now) {
        return state == State.WAITING || state == State.SENT && now - since < TIMEOUT_MS;
    }

    /** 当前状态最迟在什么时候结束（等待或未确认的超时点），空闲时为 -1。 */
    public long deadline() {
        return switch (state) {
            case IDLE -> -1;
            case WAITING -> seenAt + TIMEOUT_MS;
            case SENT -> since + TIMEOUT_MS;
        };
    }

    /** 换了输入框或连接：放弃在路上的退格。 */
    public void reset() {
        state = State.IDLE;
        seenExpectations = -1;
    }
}
