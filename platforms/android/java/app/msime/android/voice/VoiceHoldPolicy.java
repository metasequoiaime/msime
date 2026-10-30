package app.msime.android;

/**
 * 键盘里一次聆听怎样结束。长按与锁定不因停顿结束；取消、锁定和发送只命中实际按钮的矩形，其他位置松手照常发送。
 *
 * <p>纯判定，不持有计时器和视图，好在 JVM 冒烟里测。
 */
public final class VoiceHoldPolicy {
    public enum Action { SEND, CANCEL, LOCK }

    /** 屏幕坐标里的实际按钮范围；右边和下边不属于本按钮。 */
    public record Region(float left, float top, float right, float bottom) {
        public boolean contains(float x, float y) {
            return Float.isFinite(x) && Float.isFinite(y) && Float.isFinite(left)
                && Float.isFinite(top) && Float.isFinite(right) && Float.isFinite(bottom)
                && right > left && bottom > top && x >= left && x < right && y >= top && y < bottom;
        }
    }

    /** 停顿计时到点时该做的事。 */
    public enum Silence {
        /** 结束录音并出结果。 */
        STOP,
        /** VAD 还判定在说话：再等一个停顿时长。 */
        POSTPONE,
        /** 空格还按着：不自动结束，等松手。 */
        WAIT_FOR_RELEASE
    }

    private VoiceHoldPolicy() {}

    /** 非数坐标或系统取消一律取消；没有命中取消或锁定的有效坐标松开发送。「发送」按钮和「松开发送」都落在这条兜底里，所以不单独传入发送按钮的范围：它与其他未命中的位置结果相同。 */
    public static Action release(float x, float y, Region cancel, Region lock, boolean interrupted) {
        if (interrupted || !Float.isFinite(x) || !Float.isFinite(y)) return Action.CANCEL;
        if (cancel != null && cancel.contains(x, y)) return Action.CANCEL;
        if (lock != null && lock.contains(x, y)) return Action.LOCK;
        return Action.SEND;
    }

    /** 停顿计时到点：空格还按着时等松手，VAD 判定在说话时推迟，否则结束。 */
    public static Silence silenceElapsed(boolean held, boolean speaking) {
        if (held) return Silence.WAIT_FOR_RELEASE;
        return speaking ? Silence.POSTPONE : Silence.STOP;
    }
}
