package app.msime.android;

/**
 * 规定哪个进程拥有登录和匿名会话，以及其他进程如何取得令牌。
 *
 * <p>The service rotates the refresh token on every refresh and revokes the whole session when a used one is presented again. Two processes that each refresh from their own copy of the session can therefore sign the user out, and the settings app and the isolated `:ime` keyboard process both need a token. So exactly one process refreshes: the app's main process, the one sign-in and sign-out already run in（会话由 Rust 经 {@code AccountStore} 注册的存储持有）. Every other process asks it for the current access token through a non-exported provider and never reads or writes the session store itself.
 */
public final class AccountSessionRoutingPolicy {
    /** The provider authority, after the package name; the manifests declare `${applicationId}` + this. */
    public static final String AUTHORITY_SUFFIX = ".account-session";
    /** Provider 接受的 `ContentProvider.call` 方法。 */
    public static final String METHOD_ACCESS_TOKEN = "access_token";
    /** 登录的优先、否则本机匿名账号的令牌（Rust 的 access_token）；键盘进程不能直接碰匿名会话存储。 */
    public static final String METHOD_SHARED_ACCESS_TOKEN = "shared_access_token";
    /** `:ime` 进程把一次语音贡献交给主进程上传：音频是应用私有目录里的一个 WAV 文件，主进程带着会话存储调 Rust 的 voice_contribution（只用匿名账号）。 */
    public static final String METHOD_VOICE_CONTRIBUTION = "voice_contribution";
    /** `voice_contribution` 的参数：WAV 文件的绝对路径、识别语言、识别器、识别文本和应用版本。 */
    public static final String KEY_VOICE_AUDIO_PATH = "audio_path";
    public static final String KEY_VOICE_LANGUAGE = "language";
    public static final String KEY_VOICE_PROVIDER = "provider";
    public static final String KEY_VOICE_TRANSCRIPT = "transcript";
    public static final String KEY_VOICE_APP_VERSION = "app_version";
    /** `voice_contribution` 回复里服务端给的编号；失败时为空，KEY_ERROR 是 Rust 的错误码。 */
    public static final String KEY_VOICE_ID = "voice_id";
    public static final String KEY_ERROR = "error";
    /** `:ime` 写语音贡献音频的目录名，在应用的 cache 目录下；主进程只读这里面的文件。 */
    public static final String VOICE_CONTRIBUTION_DIRECTORY = "voice-contributions";
    /** `:ime` 进程读主进程记着的账号绑定（账号 ID 与绑定代次）；回复里没有任何令牌。同步开关和改动标记 `:ime` 直接经 Rust 读写，不经过这里。 */
    public static final String METHOD_SYNC_STATE = "sync_state";
    /** `sync_state` 回复中的主进程账号 ID；缺失表示暂不可读，空字符串表示未绑定。 */
    public static final String KEY_ACCOUNT_ID = "account_id";
    /** `sync_state` 回复中的账号绑定代次；缺失表示快照不可用。 */
    public static final String KEY_BINDING_GENERATION = "binding_generation";
    /** 回复中携带令牌的字段名。 */
    public static final String KEY_ACCESS_TOKEN = "access_token";
    /** 当前登录的账号编号（user.id）：刷新不变，同一个账号重新登录也不变。 */
    public static final String KEY_SESSION_ID = "session_id";
    /** Optional access token that the caller just saw rejected; the owner must not reuse it. */
    public static final String KEY_REJECTED_ACCESS_TOKEN = "rejected_access_token";
    /** The reply key saying which of the three answers this is. */
    public static final String KEY_STATE = "state";
    public static final String STATE_SIGNED_IN = "signed_in";
    public static final String STATE_SIGNED_OUT = "signed_out";
    /** The owner could not tell right now, typically a refresh that failed on the network; the caller should offer a retry rather than a sign-in. */
    public static final String STATE_UNAVAILABLE = "unavailable";

    /**
     * Which session the owning process answers from.
     *
     * <p>Two packages share this code and sign in through different paths. The native APK signs in through {@link BackendAccount}（会话由 Rust 存在 v2 存储里、在这个进程里刷新）. The Tauri combined package signs in through the shared Rust client, which keeps its session in the v1 store ({@code AccountPlugin}) and refreshes it itself. A v2 session wins when there is one; otherwise a v1 session is only ever read, never refreshed from Java, because a Java refresh would rotate the refresh token behind the Rust client's back and the service would revoke the session the next time Rust used its copy.
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

    /** 登录或匿名，总该有一枚令牌；主进程暂时拿不到时让键盘重试，不把它当成已退出。 */
    public static String sharedTokenFromReply(String state, String token) {
        if (STATE_SIGNED_IN.equals(state) && AccountTokenPolicy.validToken(token)) return token;
        throw new IllegalStateException("anonymous account unavailable");
    }

    /** Provider 只回答本 UID 的两种令牌方法、语音贡献与读账号绑定；manifest 的 `exported="false"` 是第一道边界，这里是第二道。 */
    public static boolean accepts(String method, int callingUid, int ownUid) {
        return (METHOD_ACCESS_TOKEN.equals(method) || METHOD_SHARED_ACCESS_TOKEN.equals(method)
                || METHOD_VOICE_CONTRIBUTION.equals(method) || METHOD_SYNC_STATE.equals(method))
            && callingUid == ownUid;
    }

    /** 是不是只读账号绑定、不涉及令牌的方法；provider 据此走另一条分支，回复里不放令牌字段。 */
    public static boolean syncMethod(String method) {
        return METHOD_SYNC_STATE.equals(method);
    }
}
