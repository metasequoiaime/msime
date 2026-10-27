import app.msime.android.ClipboardSelectionPolicy;

public final class ClipboardSelectionPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(ClipboardSelectionPolicy.MAX_SELECTIONS == 20, "selection cap is 20");
        check(ClipboardSelectionPolicy.canSelect(0), "empty selection can add");
        check(ClipboardSelectionPolicy.canSelect(19), "nineteen selected can add");
        check(!ClipboardSelectionPolicy.canSelect(20), "twenty selected cannot add");
        System.out.println("Android clipboard multi-selection cap passed");
    }
}
