import app.msime.android.core.InputViewValuePolicy;

public final class InputViewValuePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(InputViewValuePolicy.schemeValue(4, -1) == 4, "integer scheme is preserved");
        check(InputViewValuePolicy.schemeValue(4.5, -1) == -1,
            "fractional scheme is rejected");
        check(InputViewValuePolicy.schemeValue(true, -1) == -1,
            "boolean scheme is rejected");
        check(InputViewValuePolicy.schemeValue("4", -1) == -1,
            "string scheme is rejected");
        System.out.println("Android input view integer fields passed");
    }
}
