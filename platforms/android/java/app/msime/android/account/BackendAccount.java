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
    private static final String ORIGIN = "https://api.msime.app";
    private static final String SESSION_STORE = "msime_account_session_v2";
    private static final int MAX_RESPONSE_BYTES = 64 * 1024;
    private static final long MAX_SESSION_MILLISECONDS = AccountTokenPolicy.MAX_SESSION_SECONDS * 1000L;
    private static final Object SESSION_LOCK = new Object();
    private static FutureTask<String> refreshFlight;
    private static long sessionGeneration;

    /** One challenge, waiting for the provider's token. */
    public record Challenge(String id, String nonce) {}
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
    }

    /** Where a process that does not own the session gets its access token (see {@link AccountSessionRoutingPolicy}). */
    interface TokenSource {
        String accessToken() throws Exception;
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
        this(new AndroidAccountSessionStorage(context, SESSION_STORE), BackendAccount::httpRequest,
            AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), context.getPackageName())
                ? null : sessionOwner(context));
    }

    /** The account read and refreshed in this process, whatever process it is; only {@link AccountSessionProvider} uses this, in the main process. */
    static BackendAccount owningSession(Context context) {
        return new BackendAccount(new AndroidAccountSessionStorage(context, SESSION_STORE),
            BackendAccount::httpRequest, null);
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
        return () -> {
            Bundle reply = application.getContentResolver().call(
                uri, AccountSessionRoutingPolicy.METHOD_ACCESS_TOKEN, null, null);
            if (reply == null) throw new IllegalStateException("account session unavailable");
            return AccountSessionRoutingPolicy.tokenFromReply(
                reply.getString(AccountSessionRoutingPolicy.KEY_STATE),
                reply.getString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN));
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
            return providers().optBoolean(provider, false);
        } catch (Exception | LinkageError error) {
            return false;
        }
    }

    /** Start a sign-in and get the nonce the provider's SDK has to echo. */
    public Challenge challenge(String provider, String target) throws Exception {
        JSONObject body = new JSONObject().put("provider", provider).put("purpose", "login");
        if (target != null && !target.isEmpty()) body.put("target", target);
        JSONObject response = request("POST", "/v1/auth/challenges", body, null);
        String id = response.optString("challenge_id", "");
        String nonce = response.optString("nonce", "");
        if (id.length() != 64 || nonce.isEmpty()) {
            throw new IllegalStateException("challenge unavailable");
        }
        return new Challenge(id, nonce);
    }

    /** Finish it with the provider's ID token, and keep the session this device is now signed in on. */
    public void login(Challenge challenge, String idToken) throws Exception {
        if (ownerProcess != null) throw new IllegalStateException("account session owner");
        JSONObject tokens = request("POST", "/v1/auth/login",
            new JSONObject().put("challenge_id", challenge.id()).put("credential", idToken), null);
        String access = tokens.optString("access_token", "");
        long expires = tokens.optLong("expires_in", 0);
        if (!AccountTokenPolicy.validSession(tokens.optString("token_type", ""), access,
                tokens.optString("refresh_token", ""), expires)) {
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
        if (ownerProcess != null) {
            String token = ownerProcess.accessToken();
            return AccountTokenPolicy.validToken(token) ? token : "";
        }
        FutureTask<String> flight;
        boolean owner = false;
        synchronized (SESSION_LOCK) {
            String saved = sessions.load();
            if (saved == null) return "";
            JSONObject session = new JSONObject(saved);
            JSONObject tokens = session.getJSONObject("tokens");
            if (!AccountTokenPolicy.validSession(tokens.optString("token_type", ""),
                    tokens.optString("access_token", ""), tokens.optString("refresh_token", ""),
                    tokens.optLong("expires_in", 0))) return "";
            long now = System.currentTimeMillis();
            long expiry = session.optLong("expires_at_unix_ms", 0);
            if (expiry > now + MAX_SESSION_MILLISECONDS) return "";
            if (expiry > now + 30_000L) {
                return tokens.optString("access_token", "");
            }
            if (refreshFlight != null) {
                flight = refreshFlight;
            } else {
                long generation = sessionGeneration;
                String refresh = tokens.optString("refresh_token", "");
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
        JSONObject response = request("GET", "/v1/models", null, token);
        org.json.JSONArray data = response.optJSONArray("data");
        if (data == null || data.length() == 0 || data.length() > 64)
            throw new IllegalStateException("invalid model catalogue");
        List<ChatModel> models = new ArrayList<>();
        for (int index = 0; index < data.length(); index++) {
            JSONObject item = data.optJSONObject(index);
            String id = item == null ? "" : item.optString("id", "").trim();
            if (id.isEmpty() || id.length() > 256) throw new IllegalStateException("invalid model catalogue");
            models.add(new ChatModel(id));
        }
        return List.copyOf(models);
    }

    /** Sends one bounded non-streaming chat request; callers must run it off the UI thread. */
    public String chat(List<ChatMessage> messages, String model) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || model == null || model.isBlank() || model.length() > 256)
            throw new IllegalStateException("invalid chat request");
        if (messages == null || messages.isEmpty() || messages.size() > 14)
            throw new IllegalStateException("invalid chat request");
        org.json.JSONArray payloadMessages = new org.json.JSONArray();
        int bytes = 0;
        for (ChatMessage message : messages) {
            if (message == null || !("user".equals(message.role()) || "assistant".equals(message.role())))
                throw new IllegalStateException("invalid chat request");
            String content = message.content() == null ? "" : message.content();
            if (content.isEmpty() || content.length() > 10_000 || (bytes += TextPolicy.utf8Length(content)) > 48_000)
                throw new IllegalStateException("invalid chat request");
            payloadMessages.put(new JSONObject().put("role", message.role()).put("content", content));
        }
        JSONObject body = new JSONObject().put("messages", payloadMessages)
            .put("model", model).put("max_tokens", 2048).put("stream", false);
        JSONObject response = request("POST", "/v1/chat/completions", body, token);
        org.json.JSONArray choices = response.optJSONArray("choices");
        JSONObject first = choices == null || choices.length() == 0 ? null : choices.optJSONObject(0);
        JSONObject message = first == null ? null : first.optJSONObject("message");
        String content = message == null ? "" : message.optString("content", "");
        if (content.isEmpty() || content.length() > 10_000) throw new IllegalStateException("invalid chat response");
        return content;
    }

    public ClipboardPage clipboard(String search) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || search == null || search.length() > 1024 || TextPolicy.hasControl(search))
            throw new IllegalStateException("invalid clipboard request");
        String encoded = java.net.URLEncoder.encode(search, StandardCharsets.UTF_8.name()).replace("+", "%20");
        JSONObject response = request("GET", "/v1/users/me/clipboard?q=" + encoded, null, token);
        org.json.JSONArray values = response.optJSONArray("items");
        if (values == null || values.length() > 50) throw new IllegalStateException("invalid clipboard response");
        List<ClipboardItem> items = new ArrayList<>();
        for (int index = 0; index < values.length(); index++) {
            JSONObject item = values.optJSONObject(index);
            if (item == null) throw new IllegalStateException("invalid clipboard response");
            String id = item.optString("id", "");
            String text = item.optString("text", "");
            String updated = item.optString("updated_at", "");
            if (!id.matches("[0-9a-f]{64}") || !CloudClipboardTextPolicy.valid(text)
                    || updated.isEmpty() || updated.length() > 128
                    || TextPolicy.hasControl(updated))
                throw new IllegalStateException("invalid clipboard response");
            items.add(new ClipboardItem(id, text, updated));
        }
        return new ClipboardPage(response.optBoolean("enabled", false), List.copyOf(items));
    }

    public void setClipboardEnabled(boolean enabled) throws Exception {
        String token = accessToken();
        if (token.isEmpty()) throw new IllegalStateException("HTTP 401");
        request("PUT", "/v1/users/me/clipboard/settings", new JSONObject().put("enabled", enabled), token);
    }

    public ClipboardItem addClipboard(String text) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || !CloudClipboardTextPolicy.valid(text))
            throw new IllegalStateException("invalid clipboard request");
        JSONObject item = request("POST", "/v1/users/me/clipboard", new JSONObject().put("text", text), token);
        String id = item.optString("id", "");
        String updated = item.optString("updated_at", "");
        if (!id.matches("[0-9a-f]{64}") || updated.isEmpty() || updated.length() > 128)
            throw new IllegalStateException("invalid clipboard response");
        return new ClipboardItem(id, item.optString("text", text), updated);
    }

    public void deleteClipboard(String id) throws Exception {
        String token = accessToken();
        if (token.isEmpty() || (id != null && !id.matches("[0-9a-f]{64}")))
            throw new IllegalStateException("invalid clipboard request");
        request("DELETE", id == null ? "/v1/users/me/clipboard" : "/v1/users/me/clipboard/" + id,
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
        String access = tokens.optString("access_token", "");
        long expires = tokens.optLong("expires_in", 0);
        if (!AccountTokenPolicy.validSession(tokens.optString("token_type", ""), access,
                tokens.optString("refresh_token", ""), expires)) {
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

    private static JSONObject httpRequest(String method, String path, JSONObject body, String token)
            throws Exception {
        byte[] payload = body == null ? null : body.toString().getBytes(StandardCharsets.UTF_8);
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(ORIGIN + path).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod(method);
            connection.setConnectTimeout(30_000);
            connection.setReadTimeout(30_000);
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("User-Agent", "MSIME/Android");
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
                return new JSONObject(new String(response, StandardCharsets.UTF_8));
            }
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private static byte[] readBounded(InputStream input) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[4096];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > MAX_RESPONSE_BYTES) {
                throw new IllegalStateException("response too large");
            }
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }
}
