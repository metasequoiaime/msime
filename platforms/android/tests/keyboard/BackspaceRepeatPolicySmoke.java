import app.msime.android.BackspaceRepeatPolicy;

public final class BackspaceRepeatPolicySmoke {
    public static void main(String[] args) {
        check(BackspaceRepeatPolicy.deletesAfter(0) == 1, "the press deletes once");
        check(BackspaceRepeatPolicy.deletesAfter(-5) == 1, "negative time counts as the press");
        check(BackspaceRepeatPolicy.deletesAfter(419) == 1, "no repeat before 420 ms");
        check(BackspaceRepeatPolicy.deletesAfter(420) == 2, "the first repeat at 420 ms");
        check(BackspaceRepeatPolicy.deletesAfter(489) == 2, "next repeat waits 70 ms");
        check(BackspaceRepeatPolicy.deletesAfter(490) == 3, "second repeat at 490 ms");
        check(BackspaceRepeatPolicy.deletesAfter(1_120) == 12, "one repeat per 70 ms");
        check(BackspaceRepeatPolicy.delayOf(0) == 0 && BackspaceRepeatPolicy.delayOf(1) == 420
            && BackspaceRepeatPolicy.delayOf(2) == 490, "delay schedule");
        check(BackspaceRepeatPolicy.nextDelay(1) == 420 && BackspaceRepeatPolicy.nextDelay(2) == 70,
            "next delay");
        check(BackspaceRepeatPolicy.deletesAfter(Long.MAX_VALUE) > 0, "no overflow");
        System.out.println("Android backspace repeat: 420 ms then every 70 ms passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
