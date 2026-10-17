import app.msime.android.LocalAsrRecognizer;
import app.msime.android.VoiceHoldPolicy;

/** 键盘聆听怎样结束：按住空格说话的取消区，以及停顿计时到点时结束、推迟还是等松手。 */
public final class VoiceHoldPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        float keyboard = 800f;
        float down = 700f;
        check(!VoiceHoldPolicy.inCancelZone(down, down, keyboard), "the finger where it pressed is not cancelling");
        check(!VoiceHoldPolicy.inCancelZone(down, down - 199f, keyboard), "sliding up less than a quarter of the keyboard keeps the dictation");
        check(!VoiceHoldPolicy.inCancelZone(down, down - 200f, keyboard), "exactly a quarter is still outside the cancel zone");
        check(VoiceHoldPolicy.inCancelZone(down, down - 201f, keyboard), "sliding up past a quarter of the keyboard enters the cancel zone");
        check(VoiceHoldPolicy.inCancelZone(down, -400f, keyboard), "leaving the key upwards, above the keyboard, still cancels");
        check(!VoiceHoldPolicy.inCancelZone(down, down + 500f, keyboard), "sliding down never cancels");
        check(!VoiceHoldPolicy.inCancelZone(down, down - 201f, 0f), "an unmeasured keyboard has no cancel zone");
        check(!VoiceHoldPolicy.inCancelZone(down, down - 201f, -1f), "a negative height has no cancel zone");
        check(!VoiceHoldPolicy.inCancelZone(down, down - 201f, Float.NaN), "a NaN height has no cancel zone");
        check(!VoiceHoldPolicy.inCancelZone(Float.NaN, 0f, keyboard), "a NaN origin has no cancel zone");
        check(!VoiceHoldPolicy.inCancelZone(down, Float.NaN, keyboard), "a NaN position has no cancel zone");
        check(VoiceHoldPolicy.CANCEL_ZONE_FRACTION == 0.25f, "the cancel zone starts a quarter of the keyboard above the press");

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
