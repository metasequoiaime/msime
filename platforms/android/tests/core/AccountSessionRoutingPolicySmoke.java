import app.msime.android.AccountSessionRoutingPolicy;

public final class AccountSessionRoutingPolicySmoke {
    public static void main(String[] arguments) {
        int own = 10123;
        check(AccountSessionRoutingPolicy.accepts("sync_dirty", own, own), "our own uid may mark a section");
        check(AccountSessionRoutingPolicy.accepts("sync_state", own, own), "our own uid may read the switch");
        check(!AccountSessionRoutingPolicy.accepts("sync_dirty", own + 1, own), "another uid cannot mark");
        check(!AccountSessionRoutingPolicy.accepts("sync_state", own + 1, own), "another uid cannot read");
        check(!AccountSessionRoutingPolicy.accepts("sync_tokens", own, own), "no other sync method");
        check(AccountSessionRoutingPolicy.syncMethod("sync_dirty") && AccountSessionRoutingPolicy.syncMethod("sync_state"),
            "both sync methods take the token-free branch");
        check(!AccountSessionRoutingPolicy.syncMethod("access_token")
            && !AccountSessionRoutingPolicy.syncMethod("anonymous_access_token"), "token methods are not sync methods");
        check(!AccountSessionRoutingPolicy.KEY_SYNC_ENABLED.equals(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN)
            && !AccountSessionRoutingPolicy.KEY_LOGIN_KIND.equals(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN),
            "sync replies never reuse the token key");
        System.out.println("Android account session routing policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
