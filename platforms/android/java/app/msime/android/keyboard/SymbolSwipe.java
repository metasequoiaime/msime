package app.msime.android;

/**
 * 一次按压里「沿一个方向滑过阈值、松手输入一个符号」的状态，可滑回取消（#11822）：
 *
 * <ul>
 *   <li>沿方向的位移严格超过阈值（需要时还要纵向大于横向）就待命（{@link State#ARMED}），松手输入符号；
 *   <li>待命后沿方向的位移退回到阈值的一半以内就取消（{@link State#DISARMED}），松手什么都不输入；只看沿方向的位移，横向漂移不取消，免得斜着的拇指在两种状态间来回跳；
 *   <li>取消后再滑过阈值又待命，可以来回切换；滑过一次之后不再回到 {@link State#IDLE}，这次按压不会再变回点按、长按或别的手势，与删除键上滑快速删除相同。
 * </ul>
 *
 * <p>阈值和一半之间留出的距离就是防抖区：手指停在阈值附近时状态不会闪。坐标都是 dp，向下为正，相对按下点。字母键的 {@link GridKeyGesturePolicy} 用这里的 {@link #arms} 和 {@link #disarms}；叹号滑出 0、逗号滑出句号用实例跟踪整次按压。
 */
public final class SymbolSwipe {
    /** 方向取值：上滑和下滑都算（逗号滑出句号）。另两个取值是 {@link SwipeHintPolicy#UP} 和 {@link SwipeHintPolicy#DOWN}。 */
    public static final String EITHER = "either";

    /** 这次按压在符号滑动上处在哪一步。 */
    public enum State {
        /** 还没滑过阈值：照常是点按或长按。 */
        IDLE,
        /** 滑过阈值、此刻在待命：松手输入符号。 */
        ARMED,
        /** 待命后又滑回阈值一半以内：松手什么都不输入。 */
        DISARMED
    }

    private String direction = SwipeHintPolicy.UP;
    private float thresholdDp = SwipeHintPolicy.THRESHOLD_DP;
    private boolean vertical;
    private float downX;
    private float downY;
    private State state = State.IDLE;

    /**
     * 沿 `direction` 的位移（dp）：上滑是向上移动的距离，下滑是向下的，{@link #EITHER} 取纵向位移的绝对值；其它取值按下滑处理，与 {@link SwipeHintPolicy#swiped} 相同。
     *
     * @param dy 相对按下点的纵向位移（dp，向下为正）
     */
    public static float travel(String direction, float dy) {
        if (EITHER.equals(direction)) return Math.abs(dy);
        return SwipeHintPolicy.UP.equals(direction) ? -dy : dy;
    }

    /**
     * 这一点是否构成待命：沿方向的位移严格超过 `thresholdDp`，`vertical` 为真时纵向位移还要大于横向。
     *
     * @param direction {@link SwipeHintPolicy#UP}、{@link SwipeHintPolicy#DOWN} 或 {@link #EITHER}；null 表示不响应
     */
    public static boolean arms(String direction, float thresholdDp, float dx, float dy, boolean vertical) {
        if (direction == null || !Float.isFinite(dx) || !Float.isFinite(dy)) return false;
        return travel(direction, dy) > thresholdDp && (!vertical || Math.abs(dy) > Math.abs(dx));
    }

    /** 待命中的这一点是否取消：沿方向的位移退回到 `thresholdDp` 的一半以内（含一半）。只看沿方向的位移。 */
    public static boolean disarms(String direction, float thresholdDp, float dy) {
        if (direction == null || !Float.isFinite(dy)) return false;
        return travel(direction, dy) <= thresholdDp / 2f;
    }

    /**
     * 按下：记下起点和这次按压的规则，状态回到 {@link State#IDLE}。
     *
     * @param direction 同 {@link #arms}
     * @param thresholdDp 阈值（dp），严格大于才待命
     * @param vertical 待命是否要求纵向位移大于横向
     */
    public void start(float x, float y, String direction, float thresholdDp, boolean vertical) {
        downX = x;
        downY = y;
        this.direction = direction;
        this.thresholdDp = thresholdDp;
        this.vertical = vertical;
        state = State.IDLE;
    }

    /**
     * 手指移动到 (x, y)。只喂 `ACTION_MOVE`：抬起那一点不喂。
     *
     * @return 状态是否改变
     */
    public boolean move(float x, float y) {
        State before = state;
        float dx = x - downX;
        float dy = y - downY;
        if (state == State.ARMED) {
            if (disarms(direction, thresholdDp, dy)) state = State.DISARMED;
        } else if (arms(direction, thresholdDp, dx, dy, vertical)) {
            state = State.ARMED;
        }
        return state != before;
    }

    /** 系统取消或换一次按压：回到 {@link State#IDLE}。 */
    public void reset() {
        state = State.IDLE;
    }

    public State state() { return state; }

    /** 滑过一次阈值，这次按压归符号滑动所有（待命或已取消）。 */
    public boolean engaged() { return state != State.IDLE; }

    /** 松手是否输入符号。 */
    public boolean commits() { return state == State.ARMED; }
}
