import app.msime.android.home.SignInAttemptPolicy;

/** A sign-in row may own only one provider flow at a time. */
public final class SignInAttemptPolicySmoke {
    public static void main(String[] args) {
        check(new SignInAttemptPolicy().begin(), "the first attempt starts");
        SignInAttemptPolicy policy = new SignInAttemptPolicy();
        check(policy.begin(), "an idle row starts");
        check(!policy.begin(), "a second click is ignored while the flow is active");
        policy.finish();
        check(policy.begin(), "a completed flow permits a later attempt");
        System.out.println("Android sign-in attempt policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
