package app.msime.android;

/**
 * 键盘里一次聆听怎样结束。点语音键开始的聆听在停顿后自动结束，但经过 VAD 的整段模型（SenseVoice、离线 X-ASR 等）判定还在说话时推迟：这类模型一段说完才出文字，只看「多久没有新文字」会在用户接着说时截断。长按空格开始的是按住说话：按住期间停顿不结束，松手才结束；手指向上滑进取消区时松手作废这次录音。
 *
 * <p>纯判定，不持有计时器和视图，好在 JVM 冒烟里测。
 */
public final class VoiceHoldPolicy {
    /** 按住空格说话时，手指从按下处向上滑过键盘高度的这个比例就进入取消区。 */
    public static final float CANCEL_ZONE_FRACTION = 0.25f;

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

    /**
     * 手指是否在取消区：从按下处向上（y 变小）滑过键盘高度的 {@link #CANCEL_ZONE_FRACTION}。坐标取同一个视图里的 y；键盘高度未知（不大于 0）或坐标不是数时没有取消区，松手照常结束。
     */
    public static boolean inCancelZone(float downY, float currentY, float keyboardHeight) {
        if (!(keyboardHeight > 0f) || Float.isNaN(downY) || Float.isNaN(currentY)) return false;
        return downY - currentY > keyboardHeight * CANCEL_ZONE_FRACTION;
    }

    /** 停顿计时到点：空格还按着时等松手，VAD 判定在说话时推迟，否则结束。 */
    public static Silence silenceElapsed(boolean held, boolean speaking) {
        if (held) return Silence.WAIT_FOR_RELEASE;
        return speaking ? Silence.POSTPONE : Silence.STOP;
    }
}
