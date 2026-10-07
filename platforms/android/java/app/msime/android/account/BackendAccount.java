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
import java.util.concurrent.FutureTask;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONObject;
import app.msime.android.clipboard.CloudClipboardTextPolicy;

/**
 * 水杉账号：登录方式、挑战、登录、以及本机保存的会话。
 *
 * <p>Separate from {@link BackendAnonymousAccount}, which holds the device's own identity and needs
 * no one's permission. This one is a real account: a third party vouches for it, and the backend
 * only accepts providers it has been configured for.
 *
 * <p>The exchange is the one `docs/user-auth.md` describes: ask for a challenge, hand its `nonce` to
 * the provider's own SDK unchanged, then post the ID token back as the credential. The nonce is what
 * stops a token minted for some other app, or captured from an earlier sign-in, being replayed here.
 */
public final class BackendAccount {
    /** Maximum number of models accepted in the chat catalogue. */
    public static final int MAX_CHAT_MODELS = 33;
    /** Fixed hexadecimal length of account challenge and clipboard identifiers. */
    public static final int HEX_ID_LENGTH = 64;
    private static final String ORIGIN = "https://api.msime.app";
    private static final String SESSION_STORE = "msime_account_session_v2";
    private static final String DEFAULT_USER_AGENT = "MSIME/Android";
    /** Matches client-core's account JSON response ceiling; a full cloud clipboard page can exceed 64 KiB. */
    private static final int MAX_RESPONSE_BYTES = 1024 * 1024;
    /** 一条 AI 回复的 UTF-8 字节上限，流式与非流式相同。 */
    public static final int MAX_CHAT_REPLY_BYTES = 16 * 1024;
    /** 流式回复整个响应体的上限：最多 2048 个 token 的增量块，每块几十到一两百字节的 JSON 外壳。 */
    static final int MAX_STREAM_BYTES = 4 * 1024 * 1024;
    /** SSE 单行上限；一个增量块远小于它。 */
    static final int MAX_EVENT_LINE_BYTES = 64 * 1024;
    private static final long MAX_SESSION_MILLISECONDS = AccountTokenPolicy.MAX_SESSION_SECONDS * 1000L;
    private static final Object SESSION_LOCK = new Object();
    private static FutureTask<String> refreshFlight;
    private static long sessionGeneration;

    /** One challenge, waiting for the provider's token. */
    public record Challenge(String id, String nonce) {}
    /** 一次邮箱验证码挑战：服务端的 id、用途（login / link）与有效秒数。 */
    public record EmailChallenge(String id, String purpose, long expiresIn) {}
    public record ChatModel(String id) {}
    public record ChatMessage(String role, String content) {}
    public record ClipboardItem(String id, String text, String updatedAt) {}
    public record ClipboardPage(boolean enabled, List<ClipboardItem> items) {}

    interface SessionStore {
        String load() throws Exception;
        void save(String value) throws Exception;
        void clear() throws Exception;
    }

    interface Requester {
        JSONObject request(String method, String path, JSONObject body, String token) throws Exception;

        /** 真实账号登录请求带详细 User-Agent；不关心它的实现（测试里的内存实现）照常走四参数版本。 */
        default JSONObject request(String method, String path, JSONObject body, String token, String userAgent)
                throws Exception {
            return request(method, path, body, token);
        }
    }

    /** 走 HTTPS 的默认实现；只有登录请求会传入详细 User-Agent。 */
    private static final class HttpRequester implements Requester {
        @Override public JSONObject request(String method, String path, JSONObject body, String token)
                throws Exception {
            return httpRequest(method, path, body, token, DEFAULT_USER_AGENT);
        }

        @Override public JSONObject request(String method, String path, JSONObject body, String token,
                String userAgent) throws Exception {
            return httpRequest(method, path, body, token, userAgent);
        }
    }

    /** Where a process that does not own the session gets its access token (see {@link AccountSessionRoutingPolicy}). */
    interface TokenSource {
        String accessToken() throws Exception;

        default String accessToken(String rejectedToken) throws Exception {
            return accessToken();
        }
    }

    static final class RequestException extends IllegalStateException {
        private static final long serialVersionUID = 1L;
        final int status;

        RequestException(int status) {
            super("HTTP " + status);
            this.status = status;
        }
    }

    private final SessionStore sessions;
    private final Requester requester;
    private final TokenSource ownerProcess;

    /**
     * The account as this process may use it.
     *
     * <p>In the main process this reads and refreshes the session itself. In any other process - the `:ime` keyboard - the access token comes from the main process through {@link AccountSessionProvider}, so only one process ever spends a refresh token. Sign-in and sign-out belong to the main process.
     */
    public BackendAccount(Context context) {
        this(new AndroidAccountSessionStorage(context, SESSION_STORE), new HttpRequester(),
            AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName())
                ? null : sessionOwner(context));
    }

    /** The account read and refreshed in this process, whatever process it is; only {@link AccountSessionProvider} uses this, in the main process. */
    static BackendAccount owningSession(Context context) {
        return new BackendAccount(new AndroidAccountSessionStorage(context, SESSION_STORE),
            new HttpRequester(), null);
    }

    BackendAccount(SessionStore sessions, Requester requester) {
        this(sessions, requester, null);
    }

    BackendAccount(SessionStore sessions, Requester requester, TokenSource owner) {
        this.sessions = sessions;
        this.requester = requester;
        this.ownerProcess = owner;
    }

    private static TokenSource sessionOwner(Context context) {
        Context application = context.getApplicationContext();
        Uri uri = Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName()));
        return new TokenSource() {
            @Override public String accessToken() throws Exception {
                return accessToken(null);
            }

            @Override public String accessToken(String rejectedToken) throws Exception {
                Bundle reply = application.getContentResolver().call(
                    uri, AccountSessionRoutingPolicy.METHOD_ACCESS_TOKEN, rejectedToken, null);
                if (reply == null) throw new IllegalStateException("account session unavailable");
                return AccountSessionRoutingPolicy.tokenFromReply(
                    reply.getString(AccountSessionRoutingPolicy.KEY_STATE),
                    reply.getString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN));
            }
        };
    }

    /**
     * Which sign-in methods this deployment actually accepts.
     *
     * <p>Read before anything is offered, because a provider with no client ID configured answers
     * 503 on use. A button that is always going to fail is worse than no button.
     */
    public JSONObject providers() throws Exception {
        return request("GET", "/v1/auth/providers", null, null).getJSONObject("providers");
    }

    public boolean supports(String provider) {
        try {
            return providerEnabled(providers().opt(provider));
        } catch (Exception | LinkageError error) {
            return false;
        }
    }

    /** Provider availability is a typed server flag; reject org.json scalar coercion. */
    static boolean providerEnabled(Object value) {
        return value instanceof Boolean && (Boolean) value;
    }

    /** Start a sign-in and get the nonce the provider's SDK has to echo. */
    public Challenge challenge(String provider, String target) throws Exception {
        JSONObject body = new JSONObject().put("provider", provider).put("purpose", "login");
        if (target != null && !target.isEmpty()) body.put("target", target);
        JSONObject response = request("POST", "/v1/auth/challenges", body, null);
        String id = optionalStringField(response.opt("challenge_id"), "");
        String nonce = optionalStringField(response.opt("nonce"), "");
        if (id.length() != HEX_ID_LENGTH || nonce.isEmpty()) {
            throw new IllegalStateException("challenge unavailable");
        }
        return new Challenge(id, nonce);
    }

    /** Finish it with the provider's ID token, and keep the session this device is now signed in on. */
    public void login(Challenge challenge, String idToken) throws Exception {
        login(challenge, idToken, DEFAULT_USER_AGENT);
    }

    /** 同上，登录请求带 {@link #loginUserAgent} 生成的详细 User-Agent；后端只在登录时记下它，用来在「我的设备」里显示这台设备。 */
    public void login(Challenge challenge, String idToken, String userAgent) throws Exception {
        if (ownerProcess != null) throw new IllegalStateException("account session owner");
        keepSession(request("POST", "/v1/auth/login",
            new JSONObject().put("challenge_id", challenge.id()).put("credential", idToken), null,
            userAgent(userAgent)));
    }

    /**
     * 请服务端给这个邮箱发一封 6 位验证码。
     *
     * <p>邮箱挑战没有 nonce，所以不走 {@link #challenge}。`purpose=link` 是把邮箱绑到已登录的账号上，服务端要求最近登录过的会话，所以带上当前令牌。
     *
     * @param purpose `login` 或 `link`
     */
    public EmailChallenge requestEmailCode(String email, String purpose) throws Exception {
        String target = TextPolicy.trimmed(email);
        if (!validEmail(target)) throw new IllegalArgumentException("invalid email");
        String token = linkToken(purpose);
        JSONObject response = request("POST", "/v1/auth/challenges", new JSONObject()
            .put("provider", "email").put("target", target).put("purpose", purpose), token);
        String id = optionalStringField(response.opt("challenge_id"), "");
        long expires = AccountTokenPolicy.strictLong(response.opt("expires_in"), 0);
        if (id.length() != HEX_ID_LENGTH || expires <= 0) throw new IllegalStateException("challenge unavailable");
        return new EmailChallenge(id, purpose, expires);
    }

    /** 提交邮件里的验证码完成登录（或绑定），保存得到的会话。 */
    public void verifyEmailCode(EmailChallenge challenge, String code, String userAgent) throws Exception {
        if (ownerProcess != null) throw new IllegalStateException("account session owner");
        String credential = TextPolicy.trimmed(code);
        if (challenge == null || !validEmailCode(credential)) throw new IllegalArgumentException("invalid code");
        String token = linkToken(challenge.purpose());
        keepSession(request("POST", "/v1/auth/login",
            new JSONObject().put("challenge_id", challenge.id()).put("credential", credential), token,
            userAgent(userAgent)));
    }

    /**
     * 用 Apple 网页授权回调里的一次性授权码和本机保存的 verifier 换会话（`POST /v1/auth/apple/web/login`）。
     *
     * @param link 这次流程是不是绑定到已登录账号；是的话带上当前令牌
     */
    public void loginWithAppleGrant(String grant, String verifier, boolean link, String userAgent)
            throws Exception {
        if (ownerProcess != null) throw new IllegalStateException("account session owner");
        if (grant == null || grant.isEmpty() || grant.length() > AppleWebSignIn.MAX_GRANT_LENGTH
                || verifier == null || verifier.isEmpty()) {
            throw new IllegalArgumentException("invalid grant");
        }
        String token = link ? linkToken("link") : null;
        keepSession(request("POST", "/v1/auth/apple/web/login",
            new JSONObject().put("grant", grant).put("code_verifier", verifier), token, userAgent(userAgent)));
    }

    /** 当前登录会话的令牌，供需要「最近登录」的请求（`purpose=link`）使用；`login` 用途不带令牌。 */
    private String linkToken(String purpose) throws Exception {
        if ("login".equals(purpose)) return null;
        if (!"link".equals(purpose)) throw new IllegalArgumentException("invalid purpose");
        String token = currentAccessToken();
        if (token.isEmpty()) throw new IllegalStateException("HTTP 401");
        return token;
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
            if (code.charAt(index) < '0' || code.charAt(index) > '9') return false;
        }
        return true;
    }

    /**
     * 真实账号登录请求的 User-Agent：`msime-android/<versionName> (<Build.MODEL>; Android <Build.VERSION.RELEASE>; edition=<id>)`。
     *
     * <p>只给登录请求用：后端只在登录时把它记进会话，刷新和其他请求都发 `MSIME/Android`。
     */
    public static String loginUserAgent(Context context, String editionId) {
        String version;
        try {
            version = context.getPackageManager().getPackageInfo(context.getPackageName(), 0).versionName;
        } catch (android.content.pm.PackageManager.NameNotFoundException absent) {
            version = "";
        }
        return loginUserAgent(version, android.os.Build.MODEL, android.os.Build.VERSION.RELEASE, editionId);
    }

    /** 同上，各段由调用方给出；每段去掉控制字符和会破坏括号结构的字符，过长截断。 */
    public static String loginUserAgent(String versionName, String model, String release, String editionId) {
        return "msime-android/" + agentPart(versionName, "0") + " (" + agentPart(model, "unknown")
            + "; Android " + agentPart(release, "unknown") + "; edition=" + agentPart(editionId, "full") + ")";
    }

    private static String agentPart(String value, String fallback) {
        StringBuilder result = new StringBuilder(64);
        String raw = TextPolicy.trimmed(value);
        for (int index = 0; index < raw.length() && result.length() < 64; index++) {
            char c = raw.charAt(index);
            if (c < 0x20 || c > 0x7E || c == '(' || c == ')' || c == ';') continue;
            result.append(c);
        }
        String cleaned = TextPolicy.trimmed(result.toString());
        return cleaned.isEmpty() ? fallback : cleaned;
    }

    /** 一次登录请求失败时的 HTTP 状态；不是服务端拒绝（网络断了、响应读不出）时为 0。给登录界面分辨「验证码不对」「太频繁」「没开这种登录」。 */
    public static int failureStatus(Throwable error) {
        return error instanceof RequestException rejected ? rejected.status : 0;
    }

    private static String userAgent(String value) {
        return value == null || value.isEmpty() ? DEFAULT_USER_AGENT : value;
    }

    /** 校验并保存一次登录得到的会话。 */
    private void keepSession(JSONObject tokens) throws Exception {
        String access = optionalStringField(tokens.opt("access_token"), "");
        long expires = AccountTokenPolicy.strictSeconds(tokens.opt("expires_in"));
        if (!AccountTokenPolicy.validSession(optionalStringField(tokens.opt("token_type"), ""), access,
                optionalStringField(tokens.opt("refresh_token"), ""), expires)) {
            throw new IllegalStateException("login refused");
        }
        String saved = new JSONObject().put("tokens", tokens)
            .put("expires_at_unix_ms", expiration(expires)).toString();
        synchronized (SESSION_LOCK) {
            sessionGeneration++;
            sessions.save(saved);
        }
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

    /** 被服务端拒绝的令牌不能走未过期快路径，必须加入当前刷新单飞。 */
    String currentAccessToken(String rejectedToken) throws Exception {
        if (ownerProcess != null) {
            String token = ownerProcess.accessToken(rejectedToken);
            return AccountTokenPolicy.validToken(token) ? token : "";
        }
        FutureTask<String> flight;
        boolean owner = false;
        synchronized (SESSION_LOCK) {
            String saved = sessions.load();
            if (saved == null) return "";
            JSONObject session = new JSONObject(saved);
            JSONObject tokens = session.getJSONObject("tokens");
            if (!AccountTokenPolicy.validSession(optionalStringField(tokens.opt("token_type"), ""),
                    optionalStringField(tokens.opt("access_token"), ""), optionalStringField(tokens.opt("refresh_token"), ""),
                    AccountTokenPolicy.strictSeconds(tokens.opt("expires_in")))) return "";
            long now = System.currentTimeMillis();
            long expiry = AccountTokenPolicy.strictLong(session.opt("expires_at_unix_ms"), 0);
            if (expiry > now + MAX_SESSION_MILLISECONDS) return "";
            if (expiry > now + 30_000L
                    && !java.util.Objects.equals(rejectedToken,
                        optionalStringField(tokens.opt("access_token"), ""))) {
                return optionalStringField(tokens.opt("access_token"), "");
            }
            if (refreshFlight != null) {
                flight = refreshFlight;
            } else {
                long generation = sessionGeneration;
                String refresh = optionalStringField(tokens.opt("refresh_token"), "");
                flight = new FutureTask<>(() -> refresh(refresh, generation));
                refreshFlight = flight;
                owner = true;
            }
        }
        if (owner) {
            try {
                flight.run();
            } finally {
                synchronized (SESSION_LOCK) {
                    if (refreshFlight == flight) refreshFlight = null;
                }
            }
        }
        try {
            return flight.get();
        } catch (java.util.concurrent.ExecutionException error) {
            Throwable cause = error.getCause();
            if (cause instanceof Exception exception) throw exception;
            throw error;
        }
    }

    /** Whether this process's own store holds a session at all, expired or not. */
    boolean hasSession() throws Exception {
        synchronized (SESSION_LOCK) {
            return sessions.load() != null;
        }
    }

    public boolean signedIn() { return !accessToken().isEmpty(); }

    /** Loads the bounded model catalogue used by the keyboard tryout chat. */
    public List<ChatModel> chatModels() throws Exception {
        String token = accessToken();
        if (token.isEmpty()) throw new IllegalStateException("HTTP 401");
        JSONObject response = authorizedRequest("GET", "/v1/models", null, token);
        org.json.JSONArray data = response.optJSONArray("data");
        if (data == null || data.length() == 0 || data.length() > 64)
            throw new IllegalStateException("invalid model catalogue");
        List<ChatModel> models = new ArrayList<>(data.length());
        for (int index = 0; index < data.length(); index++) {
            JSONObject item = data.optJSONObject(index);
            String id = item == null ? "" : TextPolicy.trimmed(optionalStringField(item.opt("id"), ""));
            if (id.isEmpty() || id.length() > 256) throw new IllegalStateException("invalid model catalogue");
            models.add(new ChatModel(id));
        }
        String defaultModel = optionalStringField(response.opt("default_model"), "");
        if (!validChatModels(models, defaultModel))
            throw new IllegalStateException("invalid model catalogue");
        return List.copyOf(models);
    }

    static boolean validChatModels(List<ChatModel> models, String defaultModel) {
        if (models == null || models.isEmpty() || models.size() > MAX_CHAT_MODELS || defaultModel == null
                || defaultModel.isEmpty() || TextPolicy.utf8Length(defaultModel) > 200
                || TextPolicy.hasControl(defaultModel) || !TextPolicy.validUnicode(defaultModel))
            return false;
        java.util.HashSet<String> ids = new java.util.HashSet<>(models.size());
        boolean hasDefault = false;
        for (ChatModel model : models) {
            if (model == null || model.id() == null || model.id().isEmpty()
                    || TextPolicy.utf8Length(model.id()) > 200 || TextPolicy.hasControl(model.id())
                    || !TextPolicy.validUnicode(model.id())
                    || !ids.add(model.id())) return false;
            if (defaultModel.equals(model.id())) hasDefault = true;
        }
        return hasDefault;
    }

    static boolean validChatRequest(List<ChatMessage> messages, String model) {
        if (model == null || model.isEmpty() || TextPolicy.utf8Length(model) > 200
                || TextPolicy.hasControl(model) || !TextPolicy.validUnicode(model)
                || messages == null || messages.isEmpty() || messages.size() > 16) return false;
        int bytes = 0;
        for (ChatMessage message : messages) {
            if (message == null || !("user".equals(message.role()) || "assistant".equals(message.role())
                    || "system".equals(message.role())) || message.content() == null
                    || TextPolicy.trimmed(message.content()).isEmpty()
                    || TextPolicy.utf8Length(message.content()) > 16 * 1024
                    || TextPolicy.hasControlExceptWhitespace(message.content())
                    || !TextPolicy.validUnicode(message.content())) return false;
            bytes += TextPolicy.utf8Length(message.content());
        }
        return bytes <= 64 * 1024;
    }

    static boolean validChatResponse(String role, String content) {
        return "assistant".equals(role) && content != null && !TextPolicy.trimmed(content).isEmpty()
            && TextPolicy.utf8Length(content) <= 16 * 1024
            && !TextPolicy.hasControlExceptWhitespace(content)
            && TextPolicy.validUnicode(content);
    }

    static boolean validChatReplyText(String content) {
        return content != null && !TextPolicy.trimmed(content).isEmpty()
            && TextPolicy.utf8Length(content) <= MAX_CHAT_REPLY_BYTES
            && TextPolicy.validUnicode(content);
    }

    /** Sends one bounded non-streaming chat request; callers must run it off the UI thread. */
    public String chat(List<ChatMessage> messages, String model) throws Exception {
        String token = accessToken();
        JSONObject body = chatBody(messages, model, token).put("stream", false);
        return chatContent(authorizedRequest("POST", "/v1/chat/completions", body, token));
    }

    /**
     * 流式发送一次对话：每到一段增量就交给 {@code listener}，返回拼好的完整回复。调用方必须在后台线程上调用。
     *
     * <p>请求与 {@link #chat} 相同，只多了 `"stream": true`；服务端回 OpenAI 兼容的 SSE（`data: <chat.completion.chunk>`，以 `data: [DONE]` 结束）。不认识 stream 的旧后端回 HTTP 400，这时退回 {@link #chat}，把整段回复一次交给 listener。{@code call} 可以从任何线程取消，取消会断开连接，阻塞中的读取立刻失败。
     */
    public String chatStream(List<ChatMessage> messages, String model, ChatCall call, ChatStreamListener listener)
            throws Exception {
        String token = accessToken();
        JSONObject body = chatBody(messages, model, token).put("stream", true);
        try {
            try {
                return streamChat(body, token, call, listener);
            } catch (RequestException error) {
                if (error.status != 401) throw error;
                String fresh = currentAccessToken(token);
                if (fresh.isEmpty() || fresh.equals(token)) throw error;
                return streamChat(body, fresh, call, listener);
            }
        } catch (RequestException error) {
            if (error.status != 400) throw error;
            // 旧后端不认识 stream：退回非流式请求，整段回复一次交出去。
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            String reply = chat(messages, model);
            if (call.cancelled()) throw new CancellationException("chat cancelled");
            listener.onDelta(reply);
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
            if (open != null) new Thread(open::disconnect, "msime-chat-cancel").start();
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

    /** 校验对话请求并组装请求体（不含 stream 字段），{@link #chat} 和 {@link #chatStream} 共用同一套上限。 */
    private static JSONObject chatBody(List<ChatMessage> messages, String model, String token) throws Exception {
        if (token.isEmpty() || !validChatRequest(messages, model))
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
        return new JSONObject().put("messages", payloadMessages).put("model", model).put("max_tokens", 2048);
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
        HttpsURLConnection connection = (HttpsURLConnection) new URL(ORIGIN + "/v1/chat/completions").openConnection();
        try {
            if (!call.attach(connection)) throw new CancellationException("chat cancelled");
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            connection.setRequestProperty("Accept", "text/event-stream");
            connection.setRequestProperty("User-Agent", DEFAULT_USER_AGENT);
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
            if (status / 100 != 2) throw new RequestException(status);
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

    public ClipboardPage clipboard(String search) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || !validClipboardSearch(search))
            throw new IllegalStateException("invalid clipboard request");
        String encoded = java.net.URLEncoder.encode(search, StandardCharsets.UTF_8.name()).replace("+", "%20");
        JSONObject response = authorizedRequest("GET", "/v1/users/me/clipboard?q=" + encoded, null, token);
        org.json.JSONArray values = response.optJSONArray("items");
        if (values == null || values.length() > 50) throw new IllegalStateException("invalid clipboard response");
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
        return new ClipboardPage(requiredBooleanField(response.opt("enabled")), List.copyOf(items));
    }

    /** Whether a clipboard search fits client-core's 1024-byte text contract. */
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
        String token = accessToken();
        if (token.isEmpty()) throw new IllegalStateException("HTTP 401");
        authorizedRequest("PUT", "/v1/users/me/clipboard/settings", new JSONObject().put("enabled", enabled), token);
    }

    public ClipboardItem addClipboard(String text) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || !CloudClipboardTextPolicy.valid(text))
            throw new IllegalStateException("invalid clipboard request");
        JSONObject item = authorizedRequest("POST", "/v1/users/me/clipboard", new JSONObject().put("text", text), token);
        String id = optionalStringField(item.opt("id"), "");
        String returnedText = optionalStringField(item.opt("text"), text);
        String updated = optionalStringField(item.opt("updated_at"), "");
        ClipboardItem result = new ClipboardItem(id, returnedText, updated);
        if (!validClipboardItem(result))
            throw new IllegalStateException("invalid clipboard response");
        return result;
    }

    static boolean validClipboardItem(ClipboardItem item) {
        return item != null && item.id() != null && item.id().matches("[0-9a-f]{" + HEX_ID_LENGTH + "}")
            && CloudClipboardTextPolicy.valid(item.text()) && item.updatedAt() != null
            && !item.updatedAt().isEmpty() && TextPolicy.utf8Length(item.updatedAt()) <= 128
            && !TextPolicy.hasControl(item.updatedAt())
            && TextPolicy.validUnicode(item.updatedAt());
    }

    public void deleteClipboard(String id) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || (id != null && !id.matches("[0-9a-f]{" + HEX_ID_LENGTH + "}")))
            throw new IllegalStateException("invalid clipboard request");
        authorizedRequest("DELETE", id == null ? "/v1/users/me/clipboard" : "/v1/users/me/clipboard/" + id,
            null, token);
    }

    /** Forget the session on this device. The account itself is untouched. */
    public void signOut() {
        if (ownerProcess != null) return;
        try {
            synchronized (SESSION_LOCK) {
                sessionGeneration++;
                sessions.clear();
            }
        } catch (Exception | LinkageError error) {
            // 清除失败时会话仍可能有效，必须把失败交给界面，不能按退出成功处理。
            throw new IllegalStateException("account sign-out unavailable");
        }
    }

    private JSONObject request(String method, String path, JSONObject body, String token)
            throws Exception {
        return requester.request(method, path, body, token);
    }

    private JSONObject request(String method, String path, JSONObject body, String token, String userAgent)
            throws Exception {
        return requester.request(method, path, body, token, userAgent);
    }

    /** Retry one request after the server rejects an otherwise unexpired access token. */
    private JSONObject authorizedRequest(String method, String path, JSONObject body, String token)
            throws Exception {
        try {
            return request(method, path, body, token);
        } catch (RequestException error) {
            if (error.status != 401) throw error;
            String fresh = currentAccessToken(token);
            if (fresh.isEmpty() || fresh.equals(token)) throw error;
            return request(method, path, body, fresh);
        }
    }

    private String refresh(String refreshToken, long generation) throws Exception {
        JSONObject tokens;
        try {
            tokens = request("POST", "/v1/auth/refresh",
                new JSONObject().put("refresh_token", refreshToken), null);
        } catch (RequestException error) {
            if (error.status == 401 || error.status == 403) {
                synchronized (SESSION_LOCK) {
                    if (sessionGeneration == generation) {
                        sessionGeneration++;
                        sessions.clear();
                    }
                }
                return "";
            }
            throw error;
        }
        String access = optionalStringField(tokens.opt("access_token"), "");
        long expires = AccountTokenPolicy.strictSeconds(tokens.opt("expires_in"));
        if (!AccountTokenPolicy.validSession(optionalStringField(tokens.opt("token_type"), ""), access,
                optionalStringField(tokens.opt("refresh_token"), ""), expires)) {
            throw new IllegalStateException("refresh refused");
        }
        String saved = new JSONObject().put("tokens", tokens)
            .put("expires_at_unix_ms", expiration(expires)).toString();
        synchronized (SESSION_LOCK) {
            if (sessionGeneration != generation) return "";
            sessions.save(saved);
        }
        return access;
    }

    private static long expiration(long expiresInSeconds) {
        return Math.addExact(System.currentTimeMillis(), Math.multiplyExact(expiresInSeconds, 1000L));
    }

    private static JSONObject httpRequest(String method, String path, JSONObject body, String token,
            String userAgent) throws Exception {
        byte[] payload = body == null ? null : body.toString().getBytes(StandardCharsets.UTF_8);
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(ORIGIN + path).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod(method);
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("User-Agent", userAgent);
            if (token != null) connection.setRequestProperty("Authorization", "Bearer " + token);
            if (payload != null) {
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(payload.length);
                connection.setRequestProperty("Content-Type", "application/json");
                try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
            }
            int status = connection.getResponseCode();
            // 状态码带进消息里：503 是这个登录方式没配，401 是凭据不对，两件事不该长同一个样子。
            if (status / 100 != 2) throw new RequestException(status);
            try (InputStream input = connection.getInputStream()) {
                byte[] response = readBounded(input);
                if (response.length == 0) return new JSONObject();
                return new JSONObject(TextPolicy.utf8(response));
            }
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
