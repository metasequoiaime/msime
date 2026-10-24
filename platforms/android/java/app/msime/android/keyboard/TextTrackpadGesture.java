package app.msime.android;

/** 相对二维触控板：只计算方向步数和选区意图，按键派发与编辑器回读留在宿主。 */
public final class TextTrackpadGesture {
    public enum Tap { NONE, WORD, LINE }
    public record Steps(int horizontal, int vertical, boolean selecting) { }
    private static final Steps NONE = new Steps(0, 0, false);
    private static final int MAX_STEPS = 64;

    private final double horizontalDp;
    private final double verticalDp;
    private final long longPressMs;
    private final long doubleTapMs;
    private final double slopDp;
    private Object target;
    private double downX;
    private double downY;
    private double lastX;
    private double lastY;
    private double horizontalRemainder;
    private double verticalRemainder;
    private long downAt;
    private boolean moved;
    private boolean selecting;
    private Object tapTarget;
    private long tapAt;
    private double tapX;
    private double tapY;
    private int taps;

    public TextTrackpadGesture(float horizontalDp, float verticalDp, long longPressMs,
            long doubleTapMs, float slopDp) {
        if (!Float.isFinite(horizontalDp) || horizontalDp <= 0
                || !Float.isFinite(verticalDp) || verticalDp <= 0
                || longPressMs <= 0 || doubleTapMs <= 0 || !Float.isFinite(slopDp) || slopDp < 0)
            throw new IllegalArgumentException("Invalid trackpad gesture parameters");
        this.horizontalDp = horizontalDp;
        this.verticalDp = verticalDp;
        this.longPressMs = longPressMs;
        this.doubleTapMs = doubleTapMs;
        this.slopDp = slopDp;
    }

    public boolean active() { return target != null; }

    public void down(Object current, float xDp, float yDp, long nowMs) {
        clearActive();
        if (current == null || !finite(xDp, yDp) || nowMs < 0) {
            cancel();
            return;
        }
        target = current;
        downX = lastX = xDp;
        downY = lastY = yDp;
        downAt = nowMs;
    }

    public Steps move(Object current, int pointers, float xDp, float yDp) {
        if (!matches(current) || pointers != 1 || !finite(xDp, yDp)) {
            cancel();
            return NONE;
        }
        horizontalRemainder += xDp - lastX;
        verticalRemainder += yDp - lastY;
        lastX = xDp;
        lastY = yDp;
        moved |= Math.hypot(xDp - downX, yDp - downY) > slopDp;
        int horizontal = steps(horizontalRemainder, horizontalDp);
        int vertical = steps(verticalRemainder, verticalDp);
        if (horizontal != 0) horizontalRemainder %= horizontalDp;
        if (vertical != 0) verticalRemainder %= verticalDp;
        if (horizontal != 0 || vertical != 0) moved = true;
        return new Steps(horizontal, vertical, selecting);
    }

    /** 调用方用系统的长按时间安排计时；移动或连接变化后迟到的回调不能开启选择。 */
    public boolean longPress(Object current, long nowMs) {
        if (!matches(current)) {
            cancel();
            return false;
        }
        if (moved || selecting || nowMs < downAt || nowMs - downAt < longPressMs) return false;
        selecting = true;
        taps = 0;
        tapTarget = null;
        return true;
    }

    public Tap up(Object current, float xDp, float yDp, long nowMs, boolean tapSelection) {
        boolean isTap = matches(current) && finite(xDp, yDp) && nowMs >= downAt
            && nowMs - downAt < longPressMs && !moved && !selecting
            && Math.hypot(xDp - downX, yDp - downY) <= slopDp;
        clearActive();
        if (!tapSelection || !isTap) {
            cancel();
            return Tap.NONE;
        }
        boolean follows = current == tapTarget && nowMs >= tapAt && nowMs - tapAt <= doubleTapMs
            && Math.hypot(xDp - tapX, yDp - tapY) <= slopDp * 2;
        taps = follows && taps < 3 ? taps + 1 : 1;
        tapTarget = current;
        tapAt = nowMs;
        tapX = xDp;
        tapY = yDp;
        return taps == 2 ? Tap.WORD : taps == 3 ? Tap.LINE : Tap.NONE;
    }

    /** 隐藏、失焦、多指或输入连接变化都清除余量和连续点按状态。 */
    public void cancel() {
        clearActive();
        taps = 0;
        tapTarget = null;
    }

    private void clearActive() {
        target = null;
        horizontalRemainder = verticalRemainder = 0;
        moved = selecting = false;
    }

    private boolean matches(Object current) { return target != null && target == current; }
    private static boolean finite(float x, float y) { return Float.isFinite(x) && Float.isFinite(y); }

    private static int steps(double remainder, double step) {
        double count = remainder / step;
        return (int) Math.max(-MAX_STEPS, Math.min(MAX_STEPS, count));
    }
}
