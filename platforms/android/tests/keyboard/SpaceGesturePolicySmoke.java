import app.msime.android.SpaceGesturePolicy;
import app.msime.android.SpaceGesturePolicy.Outcome;
import app.msime.android.SpaceGesturePolicy.State;

public final class SpaceGesturePolicySmoke {
    public static void main(String[] args) {
        SpaceGesturePolicy policy = new SpaceGesturePolicy();
        policy.down(0, 100);
        check(!policy.tick(449), "449 ms is still a tap");
        check(policy.up(449) == Outcome.SPACE, "a short tap types a space");

        policy.down(1_000, 100);
        check(policy.tick(1_450), "450 ms opens voice");
        check(!policy.tick(1_500), "voice is entered once");
        check(!policy.move(1_600, 200), "dragging after voice won does not move the cursor");
        check(policy.up(1_700) == Outcome.VOICE, "voice wins once it fired");

        policy.down(2_000, 100);
        check(!policy.move(2_100, 109), "within the drag threshold");
        check(policy.move(2_200, 111), "past the threshold starts cursor movement");
        check(!policy.tick(2_600), "the long press cannot fire after the drag won");
        check(policy.state() == State.CURSOR, "cursor state holds");
        check(policy.up(2_700) == Outcome.CURSOR, "the drag wins");

        policy.down(3_000, 100);
        check(!policy.move(3_460, 200), "a move after 450 ms counts as the long press first");
        check(policy.state() == State.VOICE, "the long press came first");
        policy.cancel();
        check(policy.up(3_500) == Outcome.NONE, "cancel leaves nothing to commit");

        policy.down(4_000, 100);
        check(policy.up(4_500) == Outcome.VOICE, "release past 450 ms without a tick still counts as voice");
        System.out.println("Android space gesture: 450 ms long press vs horizontal drag, first wins passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
