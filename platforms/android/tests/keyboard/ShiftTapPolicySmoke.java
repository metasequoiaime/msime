import app.msime.android.ShiftTapPolicy;
import app.msime.android.ShiftTapPolicy.State;

public final class ShiftTapPolicySmoke {
    public static void main(String[] args) {
        ShiftTapPolicy shift = new ShiftTapPolicy();
        check(shift.state() == State.OFF && !shift.uppercase(), "starts off");
        check(shift.tap(1_000) == State.ONCE && shift.uppercase(), "one tap is one-shot");
        shift.letterTyped();
        check(shift.state() == State.OFF, "the one-shot is used up by a letter");

        check(shift.tap(2_000) == State.ONCE, "first tap");
        check(shift.tap(2_350) == State.LOCKED && shift.locked(), "a second tap within 350 ms locks");
        shift.letterTyped();
        check(shift.state() == State.LOCKED, "caps lock survives letters");
        check(shift.tap(2_400) == State.OFF, "a tap leaves caps lock");

        check(shift.tap(3_000) == State.ONCE, "first tap");
        check(shift.tap(3_351) == State.OFF, "a second tap after 350 ms turns shift off");
        check(shift.tap(3_400) == State.ONCE, "and the next tap starts over");
        shift.reset();
        check(shift.state() == State.OFF, "reset");
        check(shift.tap(3_450) == State.ONCE && shift.tap(3_500) == State.LOCKED,
            "reset forgets the earlier tap but a fresh double tap still locks");
        System.out.println("Android shift tap: one-shot, 350 ms double-tap lock and release passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
