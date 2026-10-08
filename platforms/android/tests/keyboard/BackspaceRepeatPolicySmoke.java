import app.msime.android.BackspaceRepeatPolicy;

public final class BackspaceRepeatPolicySmoke {
    public static void main(String[] args) {
        check(BackspaceRepeatPolicy.deletesAfter(0) == 1, "the press deletes once");
        check(BackspaceRepeatPolicy.deletesAfter(-5) == 1, "negative time counts as the press");
        check(BackspaceRepeatPolicy.deletesAfter(419) == 1, "no repeat before 420 ms");
        check(BackspaceRepeatPolicy.deletesAfter(420) == 2, "the first repeat at 420 ms");
        check(BackspaceRepeatPolicy.deletesAfter(489) == 2, "next repeat waits 70 ms");
        check(BackspaceRepeatPolicy.deletesAfter(490) == 3, "second repeat at 490 ms");
        // #5585：前 8 次连删每 70 ms，之后 12 次每 45 ms，再之后每 30 ms。
        check(BackspaceRepeatPolicy.deletesAfter(909) == 8, "seven 70 ms repeats before 910 ms");
        check(BackspaceRepeatPolicy.deletesAfter(910) == 9, "the eighth repeat at 910 ms");
        check(BackspaceRepeatPolicy.deletesAfter(954) == 9, "the ninth repeat waits 45 ms");
        check(BackspaceRepeatPolicy.deletesAfter(955) == 10, "the ninth repeat at 955 ms");
        check(BackspaceRepeatPolicy.deletesAfter(1_450) == 21, "the twentieth repeat at 1450 ms");
        check(BackspaceRepeatPolicy.deletesAfter(1_479) == 21, "the next repeat waits 30 ms");
        check(BackspaceRepeatPolicy.deletesAfter(1_480) == 22, "then one repeat per 30 ms");
        check(BackspaceRepeatPolicy.deletesAfter(2_450) > 50, "a two-and-a-half second hold deletes more than fifty");
        check(BackspaceRepeatPolicy.delayOf(0) == 0 && BackspaceRepeatPolicy.delayOf(1) == 420
            && BackspaceRepeatPolicy.delayOf(2) == 490 && BackspaceRepeatPolicy.delayOf(8) == 910
            && BackspaceRepeatPolicy.delayOf(9) == 955 && BackspaceRepeatPolicy.delayOf(20) == 1_450
            && BackspaceRepeatPolicy.delayOf(21) == 1_480, "delay schedule");
        for (int index = 0; index < 200; index++) {
            long at = BackspaceRepeatPolicy.delayOf(index);
            check(BackspaceRepeatPolicy.deletesAfter(at) == index + 1,
                "deletesAfter and delayOf agree at delete " + index);
            check(BackspaceRepeatPolicy.deletesAfter(at - 1) == Math.max(1, index),
                "no delete before its time at " + index);
            if (index >= 1) {
                check(BackspaceRepeatPolicy.delayOf(index + 1) - at == BackspaceRepeatPolicy.repeatInterval(index),
                    "repeatInterval is the gap after repeat " + index);
                check(BackspaceRepeatPolicy.nextDelay(index + 1) == BackspaceRepeatPolicy.repeatInterval(index),
                    "nextDelay matches repeatInterval after " + index);
            }
        }
        check(BackspaceRepeatPolicy.nextDelay(1) == 420 && BackspaceRepeatPolicy.nextDelay(2) == 70,
            "next delay");
        check(BackspaceRepeatPolicy.repeatInterval(0) == 70 && BackspaceRepeatPolicy.repeatInterval(7) == 70
            && BackspaceRepeatPolicy.repeatInterval(8) == 45 && BackspaceRepeatPolicy.repeatInterval(19) == 45
            && BackspaceRepeatPolicy.repeatInterval(20) == 30
            && BackspaceRepeatPolicy.repeatInterval(Integer.MAX_VALUE) == 30, "interval steps");
        check(BackspaceRepeatPolicy.deletesAfter(Long.MAX_VALUE) > 0, "no overflow");
        System.out.println("Android backspace repeat: 420 ms, then 70/45/30 ms steps passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
