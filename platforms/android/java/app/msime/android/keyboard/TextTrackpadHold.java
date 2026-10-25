package app.msime.android;

/** 按住工具临时打开的触摸所有权；宿主负责保存和恢复面板视图。 */
public final class TextTrackpadHold {
    public enum End { NONE, TAP, RESTORE }
    private final long delayMs;
    private final float slop;
    private Object target;
    private float downX;
    private float downY;
    private long downAt;
    private boolean active;

    public TextTrackpadHold(long delayMs, float slop) {
        if (delayMs <= 0 || !Float.isFinite(slop) || slop < 0)
            throw new IllegalArgumentException("Invalid hold parameters");
        this.delayMs = delayMs;
        this.slop = slop;
    }

    public boolean down(Object current, float x, float y, long now) {
        if (active) return false;
        target = current != null && Float.isFinite(x) && Float.isFinite(y) && now >= 0 ? current : null;
        downX = x;
        downY = y;
        downAt = now;
        return target != null;
    }

    public boolean activate(Object current, long now) {
        if (active) return false;
        if (target != current) target = null;
        if (target == null || now < downAt || now - downAt < delayMs) return false;
        active = true;
        return true;
    }

    public boolean move(Object current, int pointers, float x, float y) {
        if (target == null || target != current || pointers != 1 || !Float.isFinite(x) || !Float.isFinite(y)
                || !active && Math.hypot(x - downX, y - downY) > slop) {
            target = null;
            return false;
        }
        return true;
    }

    public End up(Object current) {
        boolean tap = target != null && target == current && !active;
        End result = cancel();
        return tap ? End.TAP : result;
    }

    public End cancel() {
        End result = active ? End.RESTORE : End.NONE;
        target = null;
        active = false;
        return result;
    }

    public boolean pending() { return target != null && !active; }
    public boolean active() { return active; }
}
