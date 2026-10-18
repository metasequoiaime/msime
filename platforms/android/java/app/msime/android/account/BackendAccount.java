package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import java.io.InputStream;
import java.net.URL;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CancellationException;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONException;
import org.json.JSONObject;
import app.msime.android.clipboard.CloudClipboardTextPolicy;

/**
 * 水杉账号：登录方式、挑战、登录，以及本机的登录会话。
 *
 * <p>会话、令牌、登录和刷新都由 Rust 持有（#8201 迁移计划 5b）：主进程里经 {@link AccountStore} 注入的存储调 {@code msime_client_account} 和 {@code msime_client_account_access_token}，`:ime` 键盘进程经 {@link AccountSessionProvider} 向主进程要令牌，自己不读也不刷新会话。这里还留着键盘的云剪贴板：端到端加密驱动在 Java（{@link CloudClipboardE2e}），请求用 Rust 给的令牌经 Java 的 HTTPS 发出；对话、云剪贴板的开关和删除都已经改调 Rust（{@link RustAccount}）。
 *
 * <p>登录的绑定按账号编号（user.id）：页面记下的「会话」就是当时的账号编号，同一个账号重新登录不算换号，与 Rust 相同。
 */
public final class BackendAccount {
    /** Fixed hexadecimal length of account challenge and clipboard identifiers. */
    public static final int HEX_ID_LENGTH = 64;
    /** 一次等待提供商凭据的挑战；link 挑战还记住发起它的账号（user.id）。 */
    public record Challenge(String id, String nonce, String purpose, String sessionId) {
        public Challenge(String id, String nonce) { this(id, nonce, "login", ""); }
    }
    /** 一次邮箱验证码挑战：服务端的 id、用途（login / link）、有效秒数，以及 link 时发起它的账号。 */
    public record EmailChallenge(String id, String purpose, long expiresIn, String sessionId) {
        public EmailChallenge(String id, String purpose, long expiresIn) {
            this(id, purpose, expiresIn, "");
        }
    }
    /** Apple 网页登录开始时 Rust 给的授权地址与 verifier，以及 link 时发起它的账号。 */
    public record AppleWebStart(String authorizationUrl, String codeVerifier, String sessionId) {}
    public record ClipboardItem(String id, String text, String updatedAt) {}
    /**
     * 键盘读到的云剪贴板：已解密的条目，以及本机的加密状态 `status`（`ready`、`pending` 或 `disabled`，见 {@link CloudClipboardApi}）和本机核对码 `code`。待批准时 `items` 为空。
     */
    public record ClipboardPage(boolean enabled, List<ClipboardItem> items, String status, String code) {
        /** 本机还在等已有设备批准：读不到，也不能上传。 */
        public boolean pending() { return CloudClipboardApi.STATUS_PENDING.equals(status); }
    }

    /** 发一个请求，不论状态码都把状态和正文交回，连不上时抛出；只给云剪贴板的端到端驱动用（{@link CloudClipboardE2e}）。默认实现走 HTTPS，固定发 `MSIME/Android`。 */
    interface Requester {
        CloudClipboardE2e.Reply exchange(String method, String path, String body, String token) throws Exception;
    }

    /**
     * 本进程从哪里拿登录会话（见 {@link AccountSessionRoutingPolicy}）：主进程是 Rust 持有的存储，其他进程是主进程的 provider。
     */
    interface Sessions {
        /** 当前登录的令牌和账号编号；没有登录时两者都为空串，现在说不清（刷新时断网、provider 连不上）时抛出。`rejectedToken` 是服务端刚拒绝的令牌，给了就不再交回它。 */
        SessionCredential session(String rejectedToken) throws Exception;

        /** 当前登录的账号编号，没有登录时为空串。主进程只读本地，不刷新、不联网。 */
        String userId() throws Exception;

        /** 跑一个 {@code msime_client_account} 操作并返回它的 value；只有持有会话的主进程能做。 */
        default Object account(JSONObject action) throws Exception {
            throw new IllegalStateException("account session owner");
        }
    }

    /** 一次登录的令牌和账号编号（user.id）；页面和请求按 `sessionId` 绑定到发起时的账号。 */
    record SessionCredential(String token, String sessionId) {}
    private static final SessionCredential NO_SESSION = new SessionCredential("", "");

    static final class RequestException extends IllegalStateException {
        private static final long serialVersionUID = 1L;
        final int status;
        /** 云剪贴板驱动报的错误码（例如 `clipboard_device_pending`，这时 `status` 为 0）；驱动没有给时为空字符串。 */
        final String code;

        RequestException(int status, String code) {
            super(code == null || code.isEmpty() ? "HTTP " + status : code);
            this.status = status;
            this.code = code == null ? "" : code;
        }
    }

    private final Sessions sessions;
    private final Requester requester;
    /** 这个进程是否持有会话：只有它能登录和退出。 */
    private final boolean owner;

    /**
     * The account as this process may use it.
     *
     * <p>主进程里会话由 Rust 持有（{@link AccountStore}）；其他进程（`:ime` 键盘）的令牌经 {@link AccountSessionProvider} 从主进程来，所以只有一个进程会花掉刷新令牌。登录和退出只在主进程里做。
     */
    public BackendAccount(Context context) {
        this(AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName())
                ? rustSessions(context) : sessionOwner(context), BackendAccount::httpExchange,
            AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName()));
    }

    /** 本进程自己持有的会话，不论是哪个进程；只有主进程里的 {@link AccountSessionProvider} 用它。 */
    static BackendAccount owningSession(Context context) {
        return new BackendAccount(rustSessions(context), BackendAccount::httpExchange, true);
    }

    BackendAccount(Sessions sessions, Requester requester) {
        this(sessions, requester, true);
    }

    BackendAccount(Sessions sessions, Requester requester, boolean owner) {
        this.sessions = sessions;
        this.requester = requester;
        this.owner = owner;
    }

    /** 主进程：令牌由 Rust 给（登录的那一份，必要时刷新），账号编号读 Rust 的 status。 */
    private static Sessions rustSessions(Context context) {
        Context application = context.getApplicationContext();
        return new Sessions() {
            @Override public SessionCredential session(String rejectedToken) throws Exception {
                if (userId().isEmpty()) return NO_SESSION;
                final JSONObject token;
                try {
                    token = AccountStore.accessToken(application, rejectedToken);
                } catch (AccountStore.Failure failure) {
                    // 刷新被服务端拒绝时 Rust 已经清掉本地会话，与没有登录相同。
                    if ("account_unauthorized".equals(failure.code)) return NO_SESSION;
                    throw failure;
                }
                // 期间退出了登录：Rust 交回的是匿名账号的令牌，不能当登录会话用。
                if (JsonPolicy.strictTrue(token.opt("anonymous"))) return NO_SESSION;
                return new SessionCredential(JsonPolicy.strictStringOrEmpty(token.opt("access_token")),
                    JsonPolicy.strictStringOrEmpty(token.opt("user_id")));
            }

            @Override public String userId() throws Exception {
                Object value = AccountStore.account(application, new JSONObject().put("operation", "status"));
                JSONObject user = value instanceof JSONObject status ? status.optJSONObject("user") : null;
                return user == null ? "" : JsonPolicy.strictStringOrEmpty(user.opt("id"));
            }

            @Override public Object account(JSONObject action) throws Exception {
                return AccountStore.account(application, action);
            }
        };
    }

    /** `:ime` 等其他进程：令牌和账号编号都由主进程的 provider 回答。 */
    private static Sessions sessionOwner(Context context) {
        Context application = context.getApplicationContext();
        Uri uri = Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName()));
        return new Sessions() {
            @Override public SessionCredential session(String rejectedToken) throws Exception {
                Bundle reply = application.getContentResolver().call(
                    uri, AccountSessionRoutingPolicy.METHOD_ACCESS_TOKEN, rejectedToken, null);
                if (reply == null) throw new IllegalStateException("account session unavailable");
                String token = AccountSessionRoutingPolicy.tokenFromReply(
                    reply.getString(AccountSessionRoutingPolicy.KEY_STATE),
                    reply.getString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN));
                if (token.isEmpty()) return NO_SESSION;
                String sessionId = reply.getString(AccountSessionRoutingPolicy.KEY_SESSION_ID);
                if (sessionId == null || sessionId.isEmpty())
                    throw new IllegalStateException("account session unavailable");
                return new SessionCredential(token, sessionId);
            }

            @Override public String userId() throws Exception {
                return session(null).sessionId();
            }
        };
    }

    /**
     * 这个部署接受哪些登录方式（Rust 的 providers，含 Android 用的 Apple 网页登录）。
     *
     * <p>给出任何登录按钮之前先读它：没有配置 client ID 的登录方式一用就回 503，注定失败的按钮不如没有。
     */
    public CloudApi.Providers providers() throws Exception {
        Object value = sessions.account(new JSONObject().put("operation", "providers"));
        if (!(value instanceof JSONObject providers)) throw new IllegalStateException("providers unavailable");
        return new CloudApi.Providers(JsonPolicy.strictTrue(providers.opt("google")),
            JsonPolicy.strictTrue(providers.opt("apple")), JsonPolicy.strictTrue(providers.opt("apple_web")),
            JsonPolicy.strictTrue(providers.opt("email")));
    }

    /** 开始 Google 登录或关联，取得 Google 必须原样回传的 nonce。link 挑战只能在发起它的同一个账号里完成。 */
    public Challenge googleChallenge(String purpose) throws Exception {
        String sessionId = linkSession(purpose);
        JSONObject challenge = object(sessions.account(new JSONObject().put("operation", "google_challenge")
            .put("purpose", purpose)));
        ensureLinkSession(purpose, sessionId);
        String id = JsonPolicy.strictStringOrEmpty(challenge.opt("challengeId"));
        String nonce = JsonPolicy.strictStringOrEmpty(challenge.opt("nonce"));
        if (id.isEmpty() || nonce.isEmpty()) throw new IllegalStateException("challenge unavailable");
        return new Challenge(id, nonce, purpose, sessionId);
    }

    /** 用 Google 的 ID token 完成登录或关联，会话由 Rust 保存。 */
    public void login(Challenge challenge, String idToken) throws Exception {
        if (!owner) throw new IllegalStateException("account session owner");
        if (challenge == null || !validPurpose(challenge.purpose()))
            throw new IllegalArgumentException("invalid challenge");
        requireLinkSession(challenge.purpose(), challenge.sessionId());
        sessions.account(new JSONObject().put("operation", "google_token_login")
            .put("challenge_id", challenge.id()).put("id_token", idToken).put("purpose", challenge.purpose()));
    }

    /**
     * 请服务端给这个邮箱发一封 6 位验证码。
     *
     * <p>`purpose=link` 是把邮箱绑到已登录的账号上，服务端要求最近登录过的会话，Rust 带着当前登录的令牌发。
     *
     * @param purpose `login` 或 `link`
     */
    public EmailChallenge requestEmailCode(String email, String purpose) throws Exception {
        String target = TextPolicy.trimmed(email);
        if (!validEmail(target)) throw new IllegalArgumentException("invalid email");
        String sessionId = linkSession(purpose);
        JSONObject challenge = object(sessions.account(new JSONObject().put("operation", "request_code")
            .put("provider", "email").put("target", target).put("purpose", purpose)));
        ensureLinkSession(purpose, sessionId);
        String id = JsonPolicy.strictStringOrEmpty(challenge.opt("challengeId"));
        long expires = NumberPolicy.strictLong(challenge.opt("expiresIn"), 0);
        if (id.isEmpty() || expires <= 0) throw new IllegalStateException("challenge unavailable");
        return new EmailChallenge(id, purpose, expires, sessionId);
    }

    /** 提交邮件里的验证码完成登录（或绑定），会话由 Rust 保存。 */
    public void verifyEmailCode(EmailChallenge challenge, String code) throws Exception {
        if (!owner) throw new IllegalStateException("account session owner");
        String credential = TextPolicy.trimmed(code);
        if (challenge == null || !validPurpose(challenge.purpose()) || !validEmailCode(credential))
            throw new IllegalArgumentException("invalid code");
        requireLinkSession(challenge.purpose(), challenge.sessionId());
        sessions.account(new JSONObject().put("operation", "login").put("challenge_id", challenge.id())
            .put("code", credential).put("purpose", challenge.purpose()));
    }

    /**
     * 开始 Apple 网页登录：Rust 生成 verifier、只把它的摘要交给服务端，返回校验过的 Apple 授权地址和 verifier。verifier 由调用方私下保存（浏览器打开期间进程可能被杀）。
     *
     * @param app 发起登录的 applicationId
     */
    public AppleWebStart appleWebStart(String app, String purpose) throws Exception {
        String sessionId = linkSession(purpose);
        JSONObject start = object(sessions.account(new JSONObject().put("operation", "apple_web_start")
            .put("app", app).put("purpose", purpose)));
        ensureLinkSession(purpose, sessionId);
        String url = JsonPolicy.strictStringOrEmpty(start.opt("authorizationUrl"));
        String verifier = JsonPolicy.strictStringOrEmpty(start.opt("codeVerifier"));
        if (url.isEmpty() || verifier.isEmpty()) throw new IllegalStateException("authorization unavailable");
        return new AppleWebStart(url, verifier, sessionId);
    }

    /**
     * 用 Apple 网页授权回调里的一次性授权码和本机保存的 verifier 完成登录或关联。
     *
     * @param expectedSessionId link 流程发起时的账号；账号已经换了时拒绝，不写入新会话
     */
    public void loginWithAppleGrant(String grant, String verifier, boolean link, String expectedSessionId)
            throws Exception {
        if (!owner) throw new IllegalStateException("account session owner");
        if (grant == null || grant.isEmpty() || grant.length() > AppleWebSignIn.MAX_GRANT_LENGTH
                || verifier == null || verifier.isEmpty()) {
            throw new IllegalArgumentException("invalid grant");
        }
        String purpose = link ? "link" : "login";
        requireLinkSession(purpose, expectedSessionId);
        sessions.account(new JSONObject().put("operation", "apple_web_login").put("grant", grant)
            .put("code_verifier", verifier).put("purpose", purpose));
    }

    private static boolean validPurpose(String purpose) {
        return "login".equals(purpose) || "link".equals(purpose);
    }

    /** link 要已经登录，返回当时的账号编号；login 不读会话，返回空串。 */
    private String linkSession(String purpose) throws Exception {
        if ("login".equals(purpose)) return "";
        if (!"link".equals(purpose)) throw new IllegalArgumentException("invalid purpose");
        String sessionId = sessions.userId();
        if (sessionId.isEmpty()) throw new AccountStore.Failure("account_unauthorized", 0);
        return sessionId;
    }

    /** link 的申请回来以后，登录的仍是发起它的那个账号。 */
    private void ensureLinkSession(String purpose, String sessionId) throws Exception {
        if ("link".equals(purpose) && !sessionId.equals(sessions.userId()))
            throw new CancellationException("account session changed");
    }

    /** link 的完成只在发起它的那个账号里做；login 不看。 */
    private void requireLinkSession(String purpose, String expectedSessionId) throws Exception {
        if (!"link".equals(purpose)) return;
        if (expectedSessionId == null || expectedSessionId.isEmpty()
                || !expectedSessionId.equals(sessions.userId()))
            throw new CancellationException("account session changed");
    }

    private static JSONObject object(Object value) {
        if (!(value instanceof JSONObject object)) throw new IllegalStateException("invalid account response");
        return object;
    }

    /** 粗查邮箱形状：去掉首尾空白后 3–254 个字符、恰好一个 @、两边都不为空、没有空白和控制字符。真正的校验在服务端。 */
    public static boolean validEmail(String email) {
        if (email == null || email.length() < 3 || email.length() > 254) return false;
        int at = email.indexOf('@');
        if (at <= 0 || at != email.lastIndexOf('@') || at == email.length() - 1) return false;
        for (int index = 0; index < email.length(); index++) {
            char c = email.charAt(index);
            if (c <= ' ' || c == 0x7F) return false;
        }
        return true;
    }

    /** 邮箱验证码是 6 位 ASCII 数字。 */
    public static boolean validEmailCode(String code) {
        if (code == null || code.length() != 6) return false;
        for (int index = 0; index < code.length(); index++) {
            if (!TextPolicy.isAsciiDigit(code.charAt(index))) return false;
        }
        return true;
    }

    /** The saved access token, or an empty string when this device is not signed in or the token cannot be had right now. */
    public String accessToken() {
        try {
            return currentAccessToken();
        } catch (Exception | LinkageError error) {
            return "";
        }
    }

    /**
     * The access token, an empty string when this device is not signed in, or an exception when that cannot be told right now (a refresh that failed on the network, a session owner that could not be reached).
     *
     * <p>For callers that word the two differently: signed out asks the user to sign in, the other asks them to retry.
     */
    String currentAccessToken() throws Exception {
        return currentAccessToken(null);
    }

    /** 被服务端拒绝的令牌不再交回，Rust 带着它强制刷新。 */
    String currentAccessToken(String rejectedToken) throws Exception {
        return currentSession(rejectedToken).token();
    }

    SessionCredential currentSession() throws Exception { return currentSession(null); }

    SessionCredential currentSession(String rejectedToken) throws Exception {
        SessionCredential answer = sessions.session(rejectedToken);
        return answer != null && AccountTokenPolicy.validToken(answer.token()) && !answer.sessionId().isEmpty()
            ? answer : NO_SESSION;
    }

    /** 本进程自己的存储里有没有登录会话，过期的也算。 */
    boolean hasSession() throws Exception {
        return !sessions.userId().isEmpty();
    }

    public boolean signedIn() { return !accessToken().isEmpty(); }

    /** 页面动作不能跨过换号：当前登录的账号编号，没有登录或读不了时为空串。 */
    public String sessionId() {
        try {
            return sessions.userId();
        } catch (Exception | LinkageError error) {
            return "";
        }
    }

    /**
     * Read only from the session that opened the keyboard panel.
     *
     * <p>经共享驱动读出并在本机解密（{@link CloudClipboardE2e}）；`search` 也在本机过滤。本机还在等批准时返回 `pending`、条目为空，不算失败。驱动报的错以 {@link RequestException} 抛出，`code` 保持驱动给的错误码。
     */
    public ClipboardPage clipboard(String search, String expectedSessionId, CloudClipboardE2e.Local local)
            throws Exception {
        SessionCredential session = currentSession();
        requireSession(session, expectedSessionId);
        if (session.token().isEmpty() || !validClipboardSearch(search))
            throw new IllegalStateException("invalid clipboard request");
        return clipboardPage(runClipboard(local, CloudClipboardE2e.Operation.list(search), session));
    }

    /** 驱动 `list` 的结果（经自建中继时形状相同，见 {@link CloudClipboardRelay}）读成键盘要的那一页；形状不对时抛出。 */
    static ClipboardPage clipboardPage(String json) throws JSONException {
        JSONObject response = new JSONObject(json);
        org.json.JSONArray values = response.optJSONArray("items");
        JSONObject e2e = response.optJSONObject("e2e");
        if (values == null || values.length() > 50 || e2e == null)
            throw new IllegalStateException("invalid clipboard response");
        String status = optionalStringField(e2e.opt("status"), "");
        String code = optionalStringField(e2e.opt("code"), "");
        if (!(CloudClipboardApi.STATUS_READY.equals(status) || CloudClipboardApi.STATUS_PENDING.equals(status)
                || CloudClipboardApi.STATUS_DISABLED.equals(status)) || !CloudClipboardApi.validCode(code))
            throw new IllegalStateException("invalid clipboard response");
        List<ClipboardItem> items = new ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            JSONObject item = values.optJSONObject(index);
            if (item == null) throw new IllegalStateException("invalid clipboard response");
            String id = optionalStringField(item.opt("id"), "");
            String text = optionalStringField(item.opt("text"), "");
            String updated = optionalStringField(item.opt("updated_at"), "");
            if (!validClipboardItem(new ClipboardItem(id, text, updated)))
                throw new IllegalStateException("invalid clipboard response");
            items.add(new ClipboardItem(id, text, updated));
        }
        return new ClipboardPage(requiredBooleanField(response.opt("enabled")), ListPolicy.copyOrEmpty(items),
            status, code);
    }

    /** 剪贴板搜索是否符合 client-services 的 1024 字节文本契约。 */
    static boolean validClipboardSearch(String search) {
        return search != null && TextPolicy.utf8Length(search) <= 1024
            && !TextPolicy.hasControl(search) && TextPolicy.validUnicode(search);
    }

    static boolean requiredBooleanField(Object value) {
        if (!(value instanceof Boolean)) throw new IllegalStateException("invalid clipboard response");
        return (Boolean) value;
    }

    static String requiredStringField(Object value) {
        if (!(value instanceof String)) throw new IllegalStateException("invalid account response");
        return (String) value;
    }

    static String optionalStringField(Object value, String fallback) {
        return value == null ? fallback : requiredStringField(value);
    }

    /** Prevent a queued keyboard upload from switching to a newly signed-in account. 在本机加密后上传；本机还在等批准时驱动报 `clipboard_device_pending`，不上传。 */
    ClipboardItem addClipboard(String text, String expectedSessionId, CloudClipboardE2e.Local local) throws Exception {
        SessionCredential session = currentSession();
        requireSession(session, expectedSessionId);
        if (session.token().isEmpty() || !CloudClipboardTextPolicy.valid(text))
            throw new IllegalStateException("invalid clipboard request");
        return clipboardItem(runClipboard(local, CloudClipboardE2e.Operation.add(text), session));
    }

    /** 驱动 `add` 的结果（经自建中继时形状相同）：新上传的那一条；形状不对时抛出。 */
    static ClipboardItem clipboardItem(String json) throws JSONException {
        JSONObject item = new JSONObject(json);
        String id = optionalStringField(item.opt("id"), "");
        String returnedText = optionalStringField(item.opt("text"), "");
        String updated = optionalStringField(item.opt("updated_at"), "");
        ClipboardItem result = new ClipboardItem(id, returnedText, updated);
        if (!validClipboardItem(result))
            throw new IllegalStateException("invalid clipboard response");
        return result;
    }

    /**
     * 经驱动跑一个云剪贴板操作：请求一律用开始时的这个会话发出（401 时换一枚令牌重试一次，换到的仍是同一个账号、而且是不同的令牌才重试），密钥串只在这个会话仍是当前会话时写入。
     */
    private String runClipboard(CloudClipboardE2e.Local local, CloudClipboardE2e.Operation operation,
            SessionCredential session) throws Exception {
        try {
            return CloudClipboardE2e.run(local, operation,
                (method, path, body) -> clipboardExchange(method, path, body, session),
                () -> ensureCurrentSession(session));
        } catch (CloudClipboardE2e.Failure failure) {
            throw new RequestException(failure.status, failure.code);
        }
    }

    private CloudClipboardE2e.Reply clipboardExchange(String method, String path, String body, SessionCredential session)
            throws Exception {
        ensureCurrentSession(session);
        CloudClipboardE2e.Reply reply = requester.exchange(method, path, body, session.token());
        ensureCurrentSession(session);
        if (reply.status() != 401) return reply;
        SessionCredential fresh = currentSession(session.token());
        if (!session.sessionId().equals(fresh.sessionId()))
            throw new CancellationException("account session changed");
        if (fresh.token().isEmpty() || fresh.token().equals(session.token())) return reply;
        reply = requester.exchange(method, path, body, fresh.token());
        ensureCurrentSession(session);
        return reply;
    }

    private static void requireSession(SessionCredential session, String expectedSessionId) {
        if (expectedSessionId != null && (expectedSessionId.isEmpty()
                || !expectedSessionId.equals(session.sessionId())))
            throw new CancellationException("account session changed");
    }

    static boolean validClipboardItem(ClipboardItem item) {
        return item != null && TextPolicy.isLowerHex(item.id(), HEX_ID_LENGTH)
            && CloudClipboardTextPolicy.valid(item.text())
            && TextPolicy.boundedNonEmpty(item.updatedAt(), 128);
    }

    /**
     * Forget the session on this device. The account itself is untouched.
     *
     * <p>Rust 的 forget 只清本地：登录会话和属于它的云剪贴板密钥串一起删掉（见 {@link AccountStore}），下一个登录的账号不会拿到上一个账号的剪贴板密钥。清不掉时抛出，界面不能按已退出处理。
     */
    public void signOut() {
        if (!owner) return;
        try {
            sessions.account(new JSONObject().put("operation", "forget"));
        } catch (Exception | LinkageError error) {
            throw new IllegalStateException("account sign-out unavailable");
        }
    }

    /** 登录的仍是请求开始时的那个账号；退出或换号时抛 {@link CancellationException}。主进程只读 Rust 的本地状态，不刷新。 */
    private void ensureCurrentSession(SessionCredential expected) throws Exception {
        if (expected.sessionId().isEmpty() || !expected.sessionId().equals(sessions.userId()))
            throw new CancellationException("account session changed");
    }

    /** 云剪贴板驱动的一次往返：非 2xx 也读出错误正文交回，正文上限是驱动能接收的 {@link CloudClipboardE2e#MAX_RESPONSE_BYTES}。 */
    private static CloudClipboardE2e.Reply httpExchange(String method, String path, String body, String token)
            throws Exception {
        byte[] payload = body == null ? null : TextPolicy.utf8Bytes(body);
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
            AccountHttpRequest.writeJson(connection, method, token, CloudApi.USER_AGENT, payload);
            int status = connection.getResponseCode();
            InputStream stream = HttpConnectionPolicy.isSuccessful(status) ? connection.getInputStream() : connection.getErrorStream();
            byte[] response = new byte[0];
            if (stream != null) {
                try (InputStream input = stream) {
                    response = HttpBodyPolicy.readBounded(input, CloudClipboardE2e.MAX_RESPONSE_BYTES);
                }
                if (response == null) throw new IllegalStateException("response too large");
            }
            return new CloudClipboardE2e.Reply(status, TextPolicy.utf8(response));
        } finally {
            if (connection != null) connection.disconnect();
        }
    }
}
