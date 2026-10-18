import app.msime.android.AccountSessionRoutingPolicy;

public final class AccountSessionRoutingPolicySmoke {
    public static void main(String[] arguments) {
        int own = 10123;
        check(AccountSessionRoutingPolicy.accepts("sync_state", own, own), "our own uid may read the binding");
        check(!AccountSessionRoutingPolicy.accepts("sync_state", own + 1, own), "another uid cannot read");
        // 记改动（sync_dirty）改由 `:ime` 直接调 Rust 的 mark_dirty：provider 不再收它，主进程里也就没有第二个写同步状态的地方。
        check(!AccountSessionRoutingPolicy.accepts("sync_dirty", own, own), "marking a section no longer goes through the provider");
        check(!AccountSessionRoutingPolicy.accepts("sync_tokens", own, own), "no other sync method");
        check(AccountSessionRoutingPolicy.syncMethod("sync_state"), "the binding read takes the token-free branch");
        check(!AccountSessionRoutingPolicy.syncMethod("sync_dirty"), "sync_dirty is not a provider method");
        check(!AccountSessionRoutingPolicy.syncMethod("access_token")
            && !AccountSessionRoutingPolicy.syncMethod("shared_access_token")
            && !AccountSessionRoutingPolicy.syncMethod("voice_contribution"), "token methods are not sync methods");
        check(!AccountSessionRoutingPolicy.KEY_ACCOUNT_ID.equals(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN)
            && !AccountSessionRoutingPolicy.KEY_BINDING_GENERATION.equals(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN),
            "binding replies never reuse the token key");
        System.out.println("Android account session routing policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
