import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardLayoutAdjustPolicy;

public final class KeyboardLayoutAdjustPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(KeyboardLayoutAdjustPolicy.chooseAxis(2, 12, null)
            == KeyboardLayoutAdjustPolicy.Axis.VERTICAL);
        check(KeyboardLayoutAdjustPolicy.chooseAxis(12, 2, null)
            == KeyboardLayoutAdjustPolicy.Axis.HORIZONTAL);
        check(KeyboardLayoutAdjustPolicy.chooseAxis(1, 1,
            KeyboardLayoutAdjustPolicy.Axis.HORIZONTAL)
            == KeyboardLayoutAdjustPolicy.Axis.HORIZONTAL);
        check(KeyboardLayoutAdjustPolicy.chooseAxis(Float.NaN, 1, null) == null);
        check(KeyboardLayoutAdjustPolicy.keySpacingFromDrag(60, 18) == 60);
        check(KeyboardLayoutAdjustPolicy.keySpacingFromDrag(60, -54) == 30);
        check(KeyboardLayoutAdjustPolicy.rowSpacingFromDrag(70, 54) == 100);
        check(KeyboardLayoutAdjustPolicy.heightFromDrag(0, -12) == 12);
        check(KeyboardLayoutAdjustPolicy.heightFromDrag(0, 60) == -12);
        check(KeyboardLayoutAdjustPolicy.heightFromDrag(Integer.MIN_VALUE, Float.POSITIVE_INFINITY)
            == KeyboardGeometry.DEFAULT_HEIGHT_ADJUSTMENT_DP);
        System.out.println("Android keyboard adjustment drag policy: axis, scale and bounds passed");
    }
}
