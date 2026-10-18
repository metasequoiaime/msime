package app.msime.android;

/** 令牌形状与 Tauri 合包旧会话的过期判断；登录会话的整体校验在 Rust（#8201 迁移计划 5b）。 */
public final class AccountTokenPolicy {
    static final long MAX_SESSION_SECONDS = 86_400L * 30;
    private static final long EXPIRY_MARGIN_MILLISECONDS = 30_000L;
    private static final long MAX_SESSION_MILLISECONDS = MAX_SESSION_SECONDS * 1000L;

    private AccountTokenPolicy() {}

    /** Backend account tokens are fixed-width lower-case hex values. */
    public static boolean validToken(String value) {
        return TextPolicy.isLowerHex(value, 64);
    }

    /** Reject missing, expired, and implausibly distant saved-session expiries. */
    static boolean validExpiry(long expiresAtUnixMillis, long nowUnixMillis) {
        if (expiresAtUnixMillis <= nowUnixMillis) return false;
        long remaining = expiresAtUnixMillis - nowUnixMillis;
        return remaining > EXPIRY_MARGIN_MILLISECONDS && remaining <= MAX_SESSION_MILLISECONDS;
    }

    /** Read a JSON number only when it is an exact signed 64-bit integer. */
    static long strictLong(Object value, long fallback) {
        return NumberPolicy.strictLong(value, fallback);
    }
}
