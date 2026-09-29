import app.msime.android.AccountTokenPolicy;

/** Account responses and saved sessions must satisfy the same bearer-token contract. */
public final class AccountTokenPolicySmoke {
    static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        String access = "a".repeat(64);
        String refresh = "b".repeat(64);
        check(AccountTokenPolicy.validSession("Bearer", access, refresh, 900),
            "valid account session");
        check(!AccountTokenPolicy.validSession("bearer", access, refresh, 900),
            "token type is case-sensitive");
        check(!AccountTokenPolicy.validSession("Bearer", "A".repeat(64), refresh, 900),
            "uppercase access token is refused");
        check(!AccountTokenPolicy.validSession("Bearer", access, "short", 900),
            "short refresh token is refused");
        check(!AccountTokenPolicy.validSession("Bearer", access, refresh, 0),
            "expired response is refused");
        check(!AccountTokenPolicy.validSession("Bearer", access, refresh, 86_400L * 30 + 1),
            "unbounded session lifetime is refused");
        System.out.println("Android account token policy passed");
    }
}
