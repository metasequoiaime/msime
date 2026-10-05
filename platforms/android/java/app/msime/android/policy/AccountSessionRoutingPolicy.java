package app.msime.android;

/**
 * 规定哪个进程拥有登录和匿名会话，以及其他进程如何取得令牌。
 *
 * <p>The service rotates the refresh token on every refresh and revokes the whole session when a used one is presented again. Two processes that each refresh from their own copy of the session can therefore sign the user out, and the settings app and the isolated `:ime` keyboard process both need a token. So exactly one process refreshes: the app's main process, the one sign-in and sign-out already run in. Every other process asks it for the current access token through a non-exported provider and never reads or writes the session store itself.
 */
public final class AccountSessionRoutingPolicy {
    /** The provider authority, after the package name; the manifests declare `${applicationId}` + this. */
    public static final String AUTHORITY_SUFFIX = ".account-session";
    /** Provider 接受的 `ContentProvider.call` 方法。 */
    public static final String METHOD_ACCESS_TOKEN = "access_token";
    /** 主进程独占的匿名账号令牌方法；键盘进程不能直接碰匿名会话存储。 */
    public static final String METHOD_ANONYMOUS_ACCESS_TOKEN = "anonymous_access_token";
    /** 回复中携带令牌的字段名。 */
    public static final String KEY_ACCESS_TOKEN = "access_token";
    /** The reply key saying which of the three answers this is. */
    public static final String KEY_STATE = "state";
    public static final String STATE_SIGNED_IN = "signed_in";
    public static final String STATE_SIGNED_OUT = "signed_out";
    /** The owner could not tell right now, typically a refresh that failed on the network; the caller should offer a retry rather than a sign-in. */
    public static final String STATE_UNAVAILABLE = "unavailable";

    /**
     * Which session the owning process answers from.
     *
     * <p>Two packages share this code and sign in through different paths. The native APK signs in through {@link BackendAccount}, which keeps its session in the v2 store and refreshes it here. The Tauri combined package signs in through the shared Rust client, which keeps its session in the v1 store ({@code AccountPlugin}) and refreshes it itself. A v2 session wins when there is one; otherwise a v1 session is only ever read, never refreshed from Java, because a Java refresh would rotate the refresh token behind the Rust client's back and the service would revoke the session the next time Rust used its copy.
     */
    public enum Source { OWN, LEGACY_READ_ONLY, NONE }

    /** Margin before expiry under which an access token is not handed out, matching the native client's refresh threshold. */
    public static final long EXPIRY_MARGIN_MILLIS = 30_000L;

    private AccountSessionRoutingPolicy() {}

    /**
     * Whether this process reads, refreshes and writes the session itself.
     *
     * <p>Only the main process, whose name is the package name. A process whose name cannot be read is treated as a secondary one: asking the provider is correct from any process, including the main one, whereas refreshing from the wrong one is what revokes the session.
     */
    public static boolean ownsSession(String processName, String packageName) {
        return processName != null && packageName != null && !packageName.isEmpty()
            && processName.equals(packageName);
    }

    public static String authority(String packageName) {
        if (packageName == null || packageName.isEmpty()) throw new IllegalArgumentException("No package name");
        return packageName + AUTHORITY_SUFFIX;
    }

    public static Source source(boolean ownSession, boolean legacySession) {
        if (ownSession) return Source.OWN;
        return legacySession ? Source.LEGACY_READ_ONLY : Source.NONE;
    }

    /** The v1 access token if it can be used as it is, or an empty string; an expiring one is left to the Rust client to refresh. */
    public static String legacyToken(String accessToken, long expiresAtUnixMillis, long nowUnixMillis) {
        if (!AccountTokenPolicy.validToken(accessToken)) return "";
        return AccountTokenPolicy.validExpiry(expiresAtUnixMillis, nowUnixMillis)
            ? accessToken : "";
    }

    /** The state the provider reports for one answer from the owning process. */
    public static String stateFor(String token) {
        return token == null || token.isEmpty() ? STATE_SIGNED_OUT : STATE_SIGNED_IN;
    }

    /**
     * The token a secondary process takes from the provider's reply: the token when signed in, an empty string when signed out.
     *
     * <p>Throws when the owner could not tell, or when the reply is not one this code wrote, so the caller can word a retry instead of a sign-in.
     */
    public static String tokenFromReply(String state, String token) {
        if (STATE_SIGNED_OUT.equals(state)) return "";
        if (STATE_SIGNED_IN.equals(state) && AccountTokenPolicy.validToken(token)) return token;
        throw new IllegalStateException("account session unavailable");
    }

    /** 匿名账号必须始终有令牌；主进程暂时拿不到时让键盘重试，不把它当成已退出。 */
    public static String anonymousTokenFromReply(String state, String token) {
        if (STATE_SIGNED_IN.equals(state) && AccountTokenPolicy.validToken(token)) return token;
        throw new IllegalStateException("anonymous account unavailable");
    }

    /** Provider 只回答本 UID 的两种令牌方法；manifest 的 `exported="false"` 是第一道边界，这里是第二道。 */
    public static boolean accepts(String method, int callingUid, int ownUid) {
        return (METHOD_ACCESS_TOKEN.equals(method) || METHOD_ANONYMOUS_ACCESS_TOKEN.equals(method))
            && callingUid == ownUid;
    }
}
