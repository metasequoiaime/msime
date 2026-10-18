package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
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
 * <p>会话、令牌、登录和刷新都由 Rust 持有（#8201 迁移计划 5b）：主进程里经 {@link AccountStore} 注入的存储调 {@code msime_client_account} 和 {@code msime_client_account_access_token}，`:ime` 键盘进程经 {@link AccountSessionProvider} 向主进程要令牌，自己不读也不刷新会话。这里还留着的对话、云剪贴板请求用 Rust 给的令牌经 Java 的 HTTPS 发出，随后几组提交改调 Rust。
 *
 * <p>登录的绑定按账号编号（user.id）：页面记下的「会话」就是当时的账号编号，同一个账号重新登录不算换号，与 Rust 相同。
 */
public final class BackendAccount {
    /** Fixed hexadecimal length of account challenge and clipboard identifiers. */
    public static final int HEX_ID_LENGTH = 64;
    /** 与 client-services 的账号 JSON 响应上限一致。云剪贴板的整页密文走 {@link #httpExchange}，上限是 {@link CloudClipboardE2e#MAX_RESPONSE_BYTES}。 */
    private static final int MAX_RESPONSE_BYTES = 1024 * 1024;
    /** 一条 AI 回复的 UTF-8 字节上限，流式与非流式相同。 */
    public static final int MAX_CHAT_REPLY_BYTES = 16 * 1024;
    /** 流式回复整个响应体的上限：最多 2048 个 token 的增量块，每块几十到一两百字节的 JSON 外壳。 */
    static final int MAX_STREAM_BYTES = 4 * 1024 * 1024;
    /** SSE 单行上限；一个增量块远小于它。 */
    static final int MAX_EVENT_LINE_BYTES = 64 * 1024;

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
    public record ChatMessage(String role, String content) {}
    public record ClipboardItem(String id, String text, String updatedAt) {}
    /**
     * 键盘读到的云剪贴板：已解密的条目，以及本机的加密状态 `status`（`ready`、`pending` 或 `disabled`，见 {@link CloudClipboardApi}）和本机核对码 `code`。待批准时 `items` 为空。
     */
    public record ClipboardPage(boolean enabled, List<ClipboardItem> items, String status, String code) {
        /** 本机还在等已有设备批准：读不到，也不能上传。 */
        public boolean pending() { return CloudClipboardApi.STATUS_PENDING.equals(status); }
    }

    interface Requester {
        JSONObject request(String method, String path, JSONObject body, String token) throws Exception;

        /** 发一个请求，不论状态码都把状态和正文交回，连不上时抛出；只给云剪贴板的端到端驱动用（{@link CloudClipboardE2e}）。 */
        default CloudClipboardE2e.Reply exchange(String method, String path, String body, String token) throws Exception {
            throw new UnsupportedOperationException("raw exchange");
        }
    }

    interface StreamRequester {
        String stream(JSONObject body, String token, ChatCall call, ChatStreamListener listener) throws Exception;
    }

    /** 走 HTTPS 的默认实现，固定发 `MSIME/Android`。 */
    private static final class HttpRequester implements Requester {
        @Override public JSONObject request(String method, String path, JSONObject body, String token)
                throws Exception {
            return httpRequest(method, path, body, token);
        }

        @Override public CloudClipboardE2e.Reply exchange(String method, String path, String body, String token)
                throws Exception {
            return httpExchange(method, path, body, token);
        }
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
        /** 错误码；只有云剪贴板驱动报的错带它（例如 `clipboard_device_pending`，这时 `status` 为 0），其余为空字符串。 */
        final String code;

        RequestException(int status) {
            this(status, "");
        }

        RequestException(int status, String code) {
            super(code == null || code.isEmpty() ? "HTTP " + status : code);
            this.status = status;
            this.code = code == null ? "" : code;
        }
    }

    private final Sessions sessions;
    private final Requester requester;
    private final StreamRequester streamer;
    /** 这个进程是否持有会话：只有它能登录和退出。 */
    private final boolean owner;

    /**
     * The account as this process may use it.
     *
     * <p>主进程里会话由 Rust 持有（{@link AccountStore}）；其他进程（`:ime` 键盘）的令牌经 {@link AccountSessionProvider} 从主进程来，所以只有一个进程会花掉刷新令牌。登录和退出只在主进程里做。
     */
    public BackendAccount(Context context) {
        this(AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName())
                ? rustSessions(context) : sessionOwner(context), new HttpRequester(), BackendAccount::streamChat,
            AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName()));
    }

    /** 本进程自己持有的会话，不论是哪个进程；只有主进程里的 {@link AccountSessionProvider} 用它。 */
    static BackendAccount owningSession(Context context) {
        return new BackendAccount(rustSessions(context), new HttpRequester(), BackendAccount::streamChat, true);
    }

    BackendAccount(Sessions sessions, Requester requester) {
        this(sessions, requester, BackendAccount::streamChat, true);
    }

    BackendAccount(Sessions sessions, Requester requester, StreamRequester streamer, boolean owner) {
        this.sessions = sessions;
        this.requester = requester;
        this.streamer = streamer;
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
     * <p>Read before anything is offered, because a provider with no client ID configured answers 503 on use. A button that is always going to fail is worse than no button.
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

    /** Whether this process's own store holds a login session at all, expired or not. */
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

    static boolean validChatRequest(List<ChatMessage> messages) {
        if (messages == null || messages.isEmpty() || messages.size() > 16) return false;
        int bytes = 0;
        for (ChatMessage message : messages) {
            if (message == null || !("user".equals(message.role()) || "assistant".equals(message.role())
                    || "system".equals(message.role()))
                    || !TextPolicy.boundedNonBlankAllowingWhitespaceControls(message.content(), 16 * 1024)) return false;
            bytes += TextPolicy.utf8Length(message.content());
        }
        return bytes <= 64 * 1024;
    }

    static boolean validChatResponse(String role, String content) {
        return "assistant".equals(role)
            && TextPolicy.boundedNonBlankAllowingWhitespaceControls(content, 16 * 1024);
    }

    static boolean validChatReplyText(String content) {
        return TextPolicy.hasText(content)
            && TextPolicy.utf8Length(content) <= MAX_CHAT_REPLY_BYTES
            && TextPolicy.validUnicode(content);
    }

    /** Sends one bounded non-streaming chat request; callers must run it off the UI thread. */
    public String chat(List<ChatMessage> messages) throws Exception {
        SessionCredential session = currentSession();
        JSONObject body = chatBody(messages, session.token()).put("stream", false);
        return chatContent(authorizedRequest("POST", "/v1/chat/completions", body, session));
    }

    /**
     * 流式发送一次对话：每到一段增量就交给 {@code listener}，返回拼好的完整回复。调用方必须在后台线程上调用。
     *
     * <p>请求与 {@link #chat} 相同，只多了 `"stream": true`；服务端回 OpenAI 兼容的 SSE（`data: <chat.completion.chunk>`，以 `data: [DONE]` 结束）。不认识 stream 的旧后端回 HTTP 400，这时退回 {@link #chat}，把整段回复一次交给 listener。{@code call} 可以从任何线程取消，取消会断开连接，阻塞中的读取立刻失败。
     */
    public String chatStream(List<ChatMessage> messages, ChatCall call, ChatStreamListener listener)
            throws Exception {
        return chatStream(messages, call, listener, null);
    }

    /** 只在页面传入的会话仍是当前会话时发送聊天流。 */
    public String chatStream(List<ChatMessage> messages, ChatCall call, ChatStreamListener listener,
            String expectedSessionId) throws Exception {
        SessionCredential session = currentSession();
        requireSession(session, expectedSessionId);
        JSONObject body = chatBody(messages, session.token()).put("stream", true);
        ChatStreamListener guarded = delta -> {
            try {
                ensureCurrentSession(session);
            } catch (Exception unavailable) {
                throw new CancellationException("account session changed");
            }
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            listener.onDelta(delta);
        };
        try {
            try {
                ensureCurrentSession(session);
                String reply = streamer.stream(body, session.token(), call, guarded);
                ensureCurrentSession(session);
                return reply;
            } catch (RequestException error) {
                if (error.status != 401) throw error;
                ensureCurrentSession(session);
                SessionCredential fresh = currentSession(session.token());
                if (!session.sessionId().equals(fresh.sessionId()))
                    throw new CancellationException("account session changed");
                if (fresh.token().isEmpty() || fresh.token().equals(session.token())) throw error;
                ensureCurrentSession(session);
                String reply = streamer.stream(body, fresh.token(), call, guarded);
                ensureCurrentSession(session);
                return reply;
            }
        } catch (RequestException error) {
            if (error.status != 400) throw error;
            // 旧后端不认识 stream：退回非流式请求，整段回复一次交出去。
            ensureCurrentSession(session);
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            String reply = chatContent(authorizedRequest("POST", "/v1/chat/completions",
                body.put("stream", false), session));
            ensureCurrentSession(session);
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            guarded.onDelta(reply);
            return reply;
        }
    }

    /** 流式回复的接收方：每到一段非空增量调用一次，在发起请求的那个后台线程上。 */
    public interface ChatStreamListener {
        void onDelta(String delta);
    }

    /** 一次流式对话的取消把手。 */
    public static final class ChatCall {
        private HttpsURLConnection connection;
        private boolean cancelled;

        /**
         * 取消这次请求：之后不再交出增量，正在进行的连接被断开。
         *
         * <p>断开放到一个短命线程上做：界面线程调用这里时，关闭 TLS 连接可能写出 close_notify，在主线程上会被 StrictMode 当成网络访问拦下。
         */
        public void cancel() {
            HttpsURLConnection open;
            synchronized (this) {
                cancelled = true;
                open = connection;
                connection = null;
            }
            if (open != null) ThreadPolicy.startNamedThread("msime-chat-cancel", open::disconnect);
        }

        public synchronized boolean cancelled() { return cancelled; }

        /** 记下正在用的连接；已经取消时返回 false，调用方不要再用它。 */
        synchronized boolean attach(HttpsURLConnection value) {
            if (cancelled) return false;
            connection = value;
            return true;
        }

        synchronized void detach(HttpsURLConnection value) {
            if (connection == value) connection = null;
        }
    }

    /**
     * 校验对话请求并组装请求体（不含 stream 字段），{@link #chat} 和 {@link #chatStream} 共用同一套上限。
     *
     * <p>请求体不带 `model`：用哪个模型由服务端决定，省略时后端用它配置的默认模型，用户在客户端选不了，客户端也不去 `/v1/models` 替用户挑一个。
     */
    private static JSONObject chatBody(List<ChatMessage> messages, String token) throws Exception {
        if (token.isEmpty() || !validChatRequest(messages))
            throw new IllegalStateException("invalid chat request");
        org.json.JSONArray payloadMessages = new org.json.JSONArray();
        for (int index = 0; index < messages.size(); index++) {
            ChatMessage message = messages.get(index);
            // system 只能是第一条：调用方用它给出回答约定（例如默认用中文），不能夹在对话中间。
            if (index > 0 && "system".equals(message.role()))
                throw new IllegalStateException("invalid chat request");
            payloadMessages.put(new JSONObject().put("role", message.role())
                .put("content", message.content()));
        }
        return new JSONObject().put("messages", payloadMessages).put("max_tokens", 2048);
    }

    /** 非流式回复里的 choices[0].message.content：按 {@link #validChatResponse} 校验。 */
    private static String chatContent(JSONObject response) {
        org.json.JSONArray choices = response.optJSONArray("choices");
        JSONObject first = choices == null || choices.length() == 0 ? null : choices.optJSONObject(0);
        JSONObject message = first == null ? null : first.optJSONObject("message");
        String content = message == null ? "" : requiredStringField(message.opt("content"));
        String role = message == null ? "" : optionalStringField(message.opt("role"), "");
        if (!validChatResponse(role, content) || !validChatReplyText(content))
            throw new IllegalStateException("invalid chat response");
        return content;
    }

    /** 一个 chat.completion.chunk 里的 choices[0].delta.content；只带 role 或 finish_reason 的块没有内容，返回空串。 */
    private static String chunkDelta(JSONObject chunk) {
        org.json.JSONArray choices = chunk.optJSONArray("choices");
        JSONObject first = choices == null || choices.length() == 0 ? null : choices.optJSONObject(0);
        JSONObject delta = first == null ? null : first.optJSONObject("delta");
        Object content = delta == null ? null : delta.opt("content");
        if (content == null || content == JSONObject.NULL) return "";
        return requiredStringField(content);
    }

    /** SSE 一行里 `data:` 字段的值（去掉冒号后一个可选空格）；不是 data 行（空行、注释、其他字段）返回 null。 */
    static String eventData(String line) {
        if (!line.startsWith("data:")) return null;
        String value = line.substring(5);
        return value.startsWith(" ") ? value.substring(1) : value;
    }

    /** 把一行 data 解成 JSON 对象；空的、不是对象或解析不了时返回 null，调用方跳过这一行。 */
    static JSONObject eventObject(String data) {
        String trimmed = TextPolicy.trimmed(data);
        if (!trimmed.startsWith("{")) return null;
        try {
            return new JSONObject(trimmed);
        } catch (org.json.JSONException malformed) {
            return null;
        }
    }

    /**
     * 按行读 SSE 响应体：以 LF 分行、去掉行尾 CR，整行凑齐后才按 UTF-8 解码，多字节字符跨读缓冲也不会被切坏。
     *
     * <p>单行超过 {@link #MAX_EVENT_LINE_BYTES} 或整个响应超过 {@link #MAX_STREAM_BYTES} 都直接失败，不让服务端把内存撑爆。
     */
    static final class EventLines {
        private final InputStream input;
        private final byte[] buffer = new byte[4096];
        private final ByteArrayOutputStream line = new ByteArrayOutputStream();
        private int position;
        private int limit;
        private long total;

        EventLines(InputStream input) {
            this.input = input;
        }

        /** 下一行，不含换行符；读到流尾时返回 null（流尾前没有换行的最后一段仍作为一行返回）。 */
        String next() throws Exception {
            line.reset();
            while (true) {
                if (position == limit) {
                    int count = input.read(buffer);
                    if (count == -1) return line.size() == 0 ? null : decode();
                    total += count;
                    if (total > MAX_STREAM_BYTES) throw new IllegalStateException("response too large");
                    position = 0;
                    limit = count;
                    continue;
                }
                byte value = buffer[position++];
                if (value == '\n') return decode();
                if (line.size() >= MAX_EVENT_LINE_BYTES) throw new IllegalStateException("event too large");
                line.write(value);
            }
        }

        private String decode() {
            byte[] bytes = line.toByteArray();
            int length = bytes.length > 0 && bytes[bytes.length - 1] == '\r' ? bytes.length - 1 : bytes.length;
            return new String(bytes, 0, length, StandardCharsets.UTF_8);
        }
    }

    /** 发一次流式请求并读完 SSE；连接前或响应头之前的非 2xx 抛 {@link RequestException}，流开始后的失败抛 {@link IllegalStateException}。 */
    private static String streamChat(JSONObject body, String token, ChatCall call, ChatStreamListener listener)
            throws Exception {
        byte[] payload = TextPolicy.utf8Bytes(body.toString());
        HttpsURLConnection connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + "/v1/chat/completions").openConnection();
        try {
            if (!call.attach(connection)) throw new CancellationException("chat cancelled");
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod("POST");
            HttpConnectionPolicy.setTimeouts(connection, 30_000, 30_000);
            connection.setRequestProperty("Accept", "text/event-stream");
            // SSE 必须逐事件到达；不请求 gzip，避免 Android 或中间代理等到压缩缓冲区满了才把增量交给读取端。
            connection.setRequestProperty("Accept-Encoding", "identity");
            connection.setRequestProperty("User-Agent", CloudApi.USER_AGENT);
            connection.setRequestProperty("Authorization", "Bearer " + token);
            connection.setDoOutput(true);
            connection.setFixedLengthStreamingMode(payload.length);
            connection.setRequestProperty("Content-Type", "application/json");
            // 连接真正建立之前取消时，cancel() 里的 disconnect() 什么也断不了，请求照样会发出去；所以在建连、发完请求体、拿到状态码这三处各看一次取消标记，尽早放弃，也不让取消后的 400/401 走进退回或重试。
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            try (OutputStream output = connection.getOutputStream()) {
                if (call.cancelled()) throw new CancellationException("chat cancelled");
                output.write(payload);
            }
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            int status = connection.getResponseCode();
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            if (!HttpConnectionPolicy.isSuccessful(status)) throw new RequestException(status);
            String type = connection.getContentType();
            try (InputStream input = connection.getInputStream()) {
                if (type == null || !TextPolicy.lowercase(type).startsWith("text/event-stream")) {
                    // 没按流式回答（例如中间层吞掉了 stream）：按普通 JSON 回复读，整段一次交出去。
                    byte[] response = readBounded(input);
                    String reply = chatContent(new JSONObject(TextPolicy.utf8(response)));
                    if (call.cancelled()) throw new CancellationException("chat cancelled");
                    listener.onDelta(reply);
                    return reply;
                }
                StringBuilder reply = new StringBuilder(MAX_CHAT_REPLY_BYTES);
                int replyBytes = 0;
                EventLines lines = new EventLines(input);
                String line;
                while ((line = lines.next()) != null) {
                    if (Thread.currentThread().isInterrupted() || call.cancelled())
                        throw new CancellationException("chat cancelled");
                    String data = eventData(line);
                    if (data == null) continue;
                    if ("[DONE]".equals(data)) {
                        if (!validChatReplyText(reply.toString()))
                            throw new IllegalStateException("invalid chat response");
                        return reply.toString();
                    }
                    // 空的或不是 JSON 对象的 data 行（例如中间层发来的空 data 行或心跳）跳过，不让一行杂音废掉已经收到的整段回复；`[DONE]` 和带 error 的对象照旧处理。
                    JSONObject chunk = eventObject(data);
                    if (chunk == null) continue;
                    if (chunk.has("error")) throw new IllegalStateException("chat stream failed");
                    String delta = chunkDelta(chunk);
                    if (delta.isEmpty()) continue;
                    if (TextPolicy.hasControlExceptWhitespace(delta)
                            || !TextPolicy.validUnicode(delta))
                        throw new IllegalStateException("invalid chat response");
                    int deltaBytes = TextPolicy.utf8Length(delta);
                    if (deltaBytes > MAX_CHAT_REPLY_BYTES - replyBytes)
                        throw new IllegalStateException("invalid chat response");
                    reply.append(delta);
                    replyBytes += deltaBytes;
                    listener.onDelta(delta);
                }
                // 没等到 [DONE] 流就断了：回复不完整，按失败处理，已经交出去的增量由调用方决定是否保留。
                throw new IllegalStateException("chat stream ended early");
            }
        } finally {
            call.detach(connection);
            connection.disconnect();
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

    public void setClipboardEnabled(boolean enabled) throws Exception {
        SessionCredential session = currentSession();
        if (session.token().isEmpty()) throw new IllegalStateException("HTTP 401");
        authorizedRequest("PUT", "/v1/users/me/clipboard/settings", new JSONObject().put("enabled", enabled), session);
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
     * 经驱动跑一个云剪贴板操作：请求一律用开始时的这个会话发出（401 时与 {@link #authorizedRequest} 一样换一枚令牌重试一次），密钥串只在这个会话仍是当前会话时写入。
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

    public void deleteClipboard(String id) throws Exception {
        SessionCredential session = currentSession();
        if (session.token().isEmpty() || (id != null && !TextPolicy.isLowerHex(id, HEX_ID_LENGTH)))
            throw new IllegalStateException("invalid clipboard request");
        authorizedRequest("DELETE", id == null ? "/v1/users/me/clipboard" : "/v1/users/me/clipboard/" + id,
            null, session);
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

    private JSONObject request(String method, String path, JSONObject body, String token)
            throws Exception {
        return requester.request(method, path, body, token);
    }

    /** Retry one request after the server rejects an otherwise unexpired access token. */
    private JSONObject authorizedRequest(String method, String path, JSONObject body, SessionCredential session)
            throws Exception {
        ensureCurrentSession(session);
        try {
            JSONObject response = request(method, path, body, session.token());
            ensureCurrentSession(session);
            return response;
        } catch (RequestException error) {
            if (error.status != 401) throw error;
            ensureCurrentSession(session);
            SessionCredential fresh = currentSession(session.token());
            if (!session.sessionId().equals(fresh.sessionId()))
                throw new CancellationException("account session changed");
            if (fresh.token().isEmpty() || fresh.token().equals(session.token())) throw error;
            JSONObject response = request(method, path, body, fresh.token());
            ensureCurrentSession(session);
            return response;
        }
    }

    /** 登录的仍是请求开始时的那个账号；退出或换号时抛 {@link CancellationException}。主进程只读 Rust 的本地状态，不刷新。 */
    private void ensureCurrentSession(SessionCredential expected) throws Exception {
        if (expected.sessionId().isEmpty() || !expected.sessionId().equals(sessions.userId()))
            throw new CancellationException("account session changed");
    }

    private static JSONObject httpRequest(String method, String path, JSONObject body, String token)
            throws Exception {
        byte[] payload = body == null ? null : body.toString().getBytes(StandardCharsets.UTF_8);
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
            AccountHttpRequest.writeJson(connection, method, token, CloudApi.USER_AGENT, payload);
            int status = connection.getResponseCode();
            if (!HttpConnectionPolicy.isSuccessful(status)) throw new RequestException(status);
            try (InputStream input = connection.getInputStream()) {
                byte[] response = readBounded(input);
                if (response.length == 0) return new JSONObject();
                return new JSONObject(TextPolicy.utf8(response));
            }
        } finally {
            if (connection != null) connection.disconnect();
        }
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

    private static byte[] readBounded(InputStream input) throws Exception {
        byte[] response = HttpBodyPolicy.readBounded(input, MAX_RESPONSE_BYTES);
        if (response == null) throw new IllegalStateException("response too large");
        return response;
    }

}
