import app.msime.android.PreferencesRevisionPolicy;

public final class PreferencesRevisionPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(PreferencesRevisionPolicy.read(12L, -1) == 12L, "integer revision is preserved");
        check(PreferencesRevisionPolicy.read(12.5, -1) == -1, "fractional revision is rejected");
        check(PreferencesRevisionPolicy.read(true, -1) == -1, "boolean revision is rejected");
        check(PreferencesRevisionPolicy.read("12", -1) == -1, "string revision is rejected");
        check(PreferencesRevisionPolicy.read(-1L, -1) == -1, "negative revision is rejected");
        System.out.println("Android preference revision precision passed");
    }
}
