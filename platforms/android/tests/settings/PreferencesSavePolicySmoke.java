import app.msime.android.PreferencesSavePolicy;

public final class PreferencesSavePolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(PreferencesSavePolicy.shouldApplyResponse(10, 10));
        check(PreferencesSavePolicy.shouldApplyResponse(10, 11));
        check(!PreferencesSavePolicy.shouldApplyResponse(11, 10));
        check(PreferencesSavePolicy.isCurrentOperation(7, 7, 3, 3, "/state", "/state"));
        check(!PreferencesSavePolicy.isCurrentOperation(6, 7, 3, 3, "/state", "/state"));
        check(!PreferencesSavePolicy.isCurrentOperation(7, 7, 2, 3, "/state", "/state"));
        check(!PreferencesSavePolicy.isCurrentOperation(7, 7, 3, 3, "/old", "/state"));
        check(PreferencesSavePolicy.isCurrentOperation(7, 7));
        check(!PreferencesSavePolicy.isCurrentOperation(6, 7));
        System.out.println("Android preference saves preserve newer reloads passed");
    }
}
