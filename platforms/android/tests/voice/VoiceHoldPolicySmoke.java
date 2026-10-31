import app.msime.android.LocalAsrRecognizer;
import app.msime.android.VoiceHoldPolicy;

/** 键盘聆听怎样结束：按住空格说话的实际按钮命中，以及停顿计时到点时结束、推迟还是等松手。 */
public final class VoiceHoldPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        VoiceHoldPolicy.Region cancel = new VoiceHoldPolicy.Region(80, 40, 140, 84);
        VoiceHoldPolicy.Region lock = new VoiceHoldPolicy.Region(20, 120, 64, 164);
        check(VoiceHoldPolicy.release(80, 40, cancel, lock, false) == VoiceHoldPolicy.Action.CANCEL,
            "取消按钮左上边缘命中");
        check(VoiceHoldPolicy.release(139, 83, cancel, lock, false) == VoiceHoldPolicy.Action.CANCEL,
            "取消按钮右下内侧命中");
        for (float[] point : new float[][] {{79, 50}, {140, 50}, {100, 39}, {100, 84}, {100, -400}})
            check(VoiceHoldPolicy.release(point[0], point[1], cancel, lock, false) == VoiceHoldPolicy.Action.SEND,
                "离开取消按钮立即恢复发送，不使用上滑距离");
        check(VoiceHoldPolicy.release(30, 130, cancel, lock, false) == VoiceHoldPolicy.Action.LOCK,
            "锁定只命中锁定按钮");
        check(VoiceHoldPolicy.release(64, 130, cancel, lock, false) == VoiceHoldPolicy.Action.SEND,
            "离开锁定按钮不再锁定");
        check(VoiceHoldPolicy.release(30, 130, cancel, lock, true) == VoiceHoldPolicy.Action.CANCEL,
            "系统取消不能误锁定");
        check(VoiceHoldPolicy.release(Float.NaN, 130, cancel, lock, false) == VoiceHoldPolicy.Action.CANCEL,
            "非数坐标不能发送");
        check(VoiceHoldPolicy.release(0, Float.POSITIVE_INFINITY, cancel, lock, false) == VoiceHoldPolicy.Action.CANCEL,
            "无穷坐标不能发送");
        check(VoiceHoldPolicy.release(30, 130, null, null, false) == VoiceHoldPolicy.Action.SEND,
            "尚未排版的按钮没有命中范围");
        // 面板最下一行与原功能行重合：🔓 | 松开发送 | 发送 同处手指所在的那一行，水平滑动不换行。
        VoiceHoldPolicy.Region rowLock = new VoiceHoldPolicy.Region(0, 300, 80, 352);
        check(VoiceHoldPolicy.release(200, 326, cancel, rowLock, false) == VoiceHoldPolicy.Action.SEND,
            "原空格位置松手发送");
        check(VoiceHoldPolicy.release(40, 326, cancel, rowLock, false) == VoiceHoldPolicy.Action.LOCK,
            "同一行水平左滑到锁定");
        check(VoiceHoldPolicy.release(360, 326, cancel, rowLock, false) == VoiceHoldPolicy.Action.SEND,
            "同一行水平右滑到发送");
        check(!new VoiceHoldPolicy.Region(10, 10, 10, 20).contains(10, 15), "零宽范围不能命中");
        check(!new VoiceHoldPolicy.Region(10, 10, 20, Float.NaN).contains(15, 15), "坏范围不能命中");

        check(VoiceHoldPolicy.silenceElapsed(false, false) == VoiceHoldPolicy.Silence.STOP,
            "a pause with nothing heard ends a tap-started dictation");
        check(VoiceHoldPolicy.silenceElapsed(false, true) == VoiceHoldPolicy.Silence.POSTPONE,
            "a VAD model still hearing speech postpones the stop instead of cutting the speaker off");
        check(VoiceHoldPolicy.silenceElapsed(true, false) == VoiceHoldPolicy.Silence.WAIT_FOR_RELEASE,
            "a pause never ends a dictation while the space bar is held");
        check(VoiceHoldPolicy.silenceElapsed(true, true) == VoiceHoldPolicy.Silence.WAIT_FOR_RELEASE,
            "holding the space bar wins over the VAD state");

        // 识别窗口与键盘都用 lambda 传部分结果；说话状态是默认方法，不实现它的调用方照常编译、不受影响。
        StringBuilder seen = new StringBuilder();
        LocalAsrRecognizer.Listener partialOnly = seen::append;
        partialOnly.onPartial("你好");
        partialOnly.onSpeech(true);
        partialOnly.onSpeech(false);
        check(seen.toString().equals("你好"), "a partial-only listener ignores the speech state");
        boolean[] speaking = new boolean[1];
        LocalAsrRecognizer.Listener both = new LocalAsrRecognizer.Listener() {
            @Override public void onPartial(String text) { }
            @Override public void onSpeech(boolean value) { speaking[0] = value; }
        };
        both.onSpeech(true);
        check(speaking[0], "a listener that follows the VAD receives the speech state");
        System.out.println("VoiceHoldPolicySmoke passed");
    }
}
