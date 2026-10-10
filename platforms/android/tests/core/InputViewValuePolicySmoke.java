import app.msime.android.NumberPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.core.InputViewValuePolicy;

public final class InputViewValuePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        check(NumberPolicy.strictInt(4, -1) == 4, "integer scheme is preserved");
        check(NumberPolicy.strictInt(4.5, -1) == -1,
            "fractional scheme is rejected");
        check(NumberPolicy.strictInt(true, -1) == -1,
            "boolean scheme is rejected");
        check(NumberPolicy.strictInt("4", -1) == -1,
            "string scheme is rejected");
        check(InputViewValuePolicy.integer(null, "missing", 7) == 7,
            "missing integer uses fallback");
        check(NumberPolicy.strictInt(46, 0) == 46, "ASCII replacement is preserved");
        check(NumberPolicy.strictInt(46.5, 0) == 0, "fractional ASCII is rejected");
        check(!InputViewValuePolicy.booleanValue("true", false),
            "string booleans are rejected");
        check(InputViewValuePolicy.booleanValue(Boolean.TRUE, false),
            "JSON booleans are accepted");
        check("kana".equals(InputViewValuePolicy.text("kana")),
            "JSON text is preserved");
        check(InputViewValuePolicy.text(42).isEmpty(),
            "numeric text is not converted to text");
        check(InputViewValuePolicy.text(null).isEmpty(),
            "missing text is empty");
        check("fallback".equals(InputViewValuePolicy.textOr(42, "fallback")),
            "invalid text uses the supplied fallback");
        check("".equals(InputViewValuePolicy.textOr("", "fallback")),
            "an explicit empty string remains empty");
        System.out.println("Android input view fields passed");
    }
}
