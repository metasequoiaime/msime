package app.msime.client;

/** Shared validation for account bearer tokens received from the backend or secure storage. */
public final class AccountTokenPolicy {
    static final long MAX_SESSION_SECONDS = 86_400L * 30;

    private AccountTokenPolicy() {}

    /** Backend account tokens are fixed-width lower-case hex values. */
    public static boolean validToken(String value) {
        return value != null && value.matches("[0-9a-f]{64}");
    }

    /** The complete token envelope required before a session can be persisted or used. */
    public static boolean validSession(String tokenType, String accessToken, String refreshToken,
            long expiresIn) {
        return "Bearer".equals(tokenType) && expiresIn > 0 && expiresIn <= MAX_SESSION_SECONDS
            && validToken(accessToken) && validToken(refreshToken);
    }
}
