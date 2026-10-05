import app.msime.android.AccountSessionRoutingPolicy;

public final class BackendAnonymousAccountSmoke {
    public static void main(String[] args) {
        check(AccountSessionRoutingPolicy.needsReauthentication("a".repeat(64), "a".repeat(64)),
            "a server-rejected anonymous token must not use the unexpired fast path");
        check(!AccountSessionRoutingPolicy.needsReauthentication("a".repeat(64), "b".repeat(64)),
            "a different token means another process already refreshed the session");
        check(!AccountSessionRoutingPolicy.needsReauthentication("a".repeat(64), null),
            "an ordinary access-token request keeps the fast path");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
