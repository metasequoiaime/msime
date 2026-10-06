import app.msime.android.core.InputViewValuePolicy;

public final class InputViewValuePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        check(InputViewValuePolicy.schemeValue(4, -1) == 4, "integer scheme is preserved");
        check(InputViewValuePolicy.schemeValue(4.5, -1) == -1,
            "fractional scheme is rejected");
        check(InputViewValuePolicy.schemeValue(true, -1) == -1,
            "boolean scheme is rejected");
        check(InputViewValuePolicy.schemeValue("4", -1) == -1,
            "string scheme is rejected");
        check(InputViewValuePolicy.integer(null, "missing", 7) == 7,
            "missing integer uses fallback");
        check(InputViewValuePolicy.integer(46, 0) == 46, "ASCII replacement is preserved");
        check(InputViewValuePolicy.integer(46.5, 0) == 0, "fractional ASCII is rejected");
        check(!InputViewValuePolicy.booleanValue("true", false),
            "string booleans are rejected");
        check(InputViewValuePolicy.booleanValue(Boolean.TRUE, false),
            "JSON booleans are accepted");
        System.out.println("Android input view fields passed");
    }
}
