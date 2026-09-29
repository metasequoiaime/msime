import app.msime.android.FullWidthInputPolicy;

public final class FullWidthInputPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check("　Ａ！～９".equals(FullWidthInputPolicy.output(" A!~9", true)),
            "printable ASCII should become fullwidth");
        check("中文，🙂\n".equals(FullWidthInputPolicy.output("中文，🙂\n", true)),
            "non-ASCII text and controls should remain unchanged");
        check(" A!~9".equals(FullWidthInputPolicy.output(" A!~9", false)),
            "disabled mode should preserve direct input");
        check(FullWidthInputPolicy.output(null, true) == null,
            "null should remain null");
        System.out.println("Android full-width input policy passed");
    }
}
