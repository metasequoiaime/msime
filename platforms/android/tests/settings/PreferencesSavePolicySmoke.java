import app.msime.android.PreferencesSavePolicy;

public final class PreferencesSavePolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(PreferencesSavePolicy.shouldApplyResponse(10, 10));
        check(PreferencesSavePolicy.shouldApplyResponse(10, 11));
        check(!PreferencesSavePolicy.shouldApplyResponse(11, 10));
        check(PreferencesSavePolicy.accepted(Boolean.TRUE));
        check(!PreferencesSavePolicy.accepted("true"));
        check(!PreferencesSavePolicy.accepted(Integer.valueOf(1)));
        System.out.println("Android preference saves preserve newer reloads passed");
    }
}
