package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Locale;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONObject;

/** Creates and persists the keyboard-only anonymous backend identity. */
final class BackendAnonymousAccount {
    /**
     * The login endpoint is rate limiting; this is not a missing account.
     *
     * <p>Separate from the generic failure because the two want opposite handling: a rate limit
     * clears on its own and must be waited out, while everything else is worth reporting.
     */
    static final class RateLimited extends IllegalStateException {
        private static final long serialVersionUID = 1L;
        private final long retryAfterMillis;

        RateLimited(long retryAfterMillis) {
            super("anonymous login rate limited");
            this.retryAfterMillis = Math.max(0, retryAfterMillis);
        }

        long retryAfterMillis() { return retryAfterMillis; }
    }

    private static final String ORIGIN = "https://api.msime.app";
    private static final String SESSION_STORE = "msime_anonymous_session_v1";
    private static final String CREDENTIAL_STORE = "msime_anonymous_account_v1";
    private static final int MAX_RESPONSE_BYTES = 64 * 1024;
    private static final SecureRandom RANDOM = new SecureRandom();
    private static final Object LOCK = new Object();
    /**
     * When the login endpoint may be asked again.
     *
     * <p>A 429 carries `Retry-After`, and honouring it is not politeness: every catalogue page,
     * every search and every tab switch asks for a token, so without a gate the host answers a
     * rate limit by immediately spending another request against it, and the window never clears.
     * Static because the gate belongs to the endpoint, not to one instance of this class.
     */
    private static long nextAttemptAtMillis;
    private final AndroidAccountSessionStorage sessions;
    private final AndroidAccountSessionStorage credentials;
    private final Context application;

    BackendAnonymousAccount(Context context) {
        application = context.getApplicationContext();
        sessions = new AndroidAccountSessionStorage(application, SESSION_STORE);
        credentials = new AndroidAccountSessionStorage(application, CREDENTIAL_STORE);
    }

    String accessToken() throws Exception {
        return accessToken(null);
    }

    /** Return a token, forcing anonymous re-authentication when the supplied token was rejected. */
    String accessToken(String rejectedToken) throws Exception {
        if (!AccountSessionRoutingPolicy.ownsSession(
                Application.getProcessName(), application.getPackageName())) {
            return ownerToken(rejectedToken);
        }
        synchronized (LOCK) {
            String saved = sessions.load();
            if (saved != null) {
                String token = tokenFromSession(saved);
                if (token != null && !AccountSessionRoutingPolicy.needsReauthentication(token, rejectedToken)) {
                    return token;
                }
            }
            // 身份先落盘再谈联网：账号是本机自己生成的，不需要后端点头，后端只是发令牌的。
            JSONObject identity = loadOrCreateIdentity();
            if (System.currentTimeMillis() < nextAttemptAtMillis) {
                throw new RateLimited(nextAttemptAtMillis - System.currentTimeMillis());
            }
            JSONObject challenge = request("POST", "/v1/auth/challenges",
                new JSONObject().put("provider", "anonymous")
                    .put("target", identity.getString("subject"))
                    .put("purpose", "login"), null);
            String challengeID = BackendAccount.optionalStringField(challenge.opt("challenge_id"), "");
            if (challengeID.isEmpty() || TextPolicy.hasControl(challengeID)) {
                throw new IllegalStateException("anonymous account unavailable");
            }
            JSONObject tokens = request("POST", "/v1/auth/login",
                new JSONObject().put("challenge_id", challengeID)
                    .put("credential", identity.getString("secret")), null);
            String tokenValue = BackendAccount.optionalStringField(tokens.opt("access_token"), "");
            String refreshValue = BackendAccount.optionalStringField(tokens.opt("refresh_token"), "");
            String tokenType = BackendAccount.optionalStringField(tokens.opt("token_type"), "");
            String token = AccountTokenPolicy.validToken(tokenValue) ? tokenValue : null;
            String refresh = AccountTokenPolicy.validToken(refreshValue) ? refreshValue : null;
            if (token == null || refresh == null || !"Bearer".equals(tokenType))
                throw new IllegalStateException("anonymous account unavailable");
            long expires = AccountTokenPolicy.strictSeconds(tokens.opt("expires_in"));
            if (!AccountTokenPolicy.validSession(tokenType, token,
                    refresh, expires)) throw new IllegalStateException("anonymous account unavailable");
            JSONObject savedSession = new JSONObject().put("tokens", tokens)
                .put("expires_at_unix_ms", System.currentTimeMillis() + expires * 1000L);
            sessions.save(savedSession.toString());
            return token;
        }
    }

    /** 从主进程取匿名令牌，避免跨进程 SharedPreferences 缓存和 refresh rotation 竞态。 */
    private String ownerToken(String rejectedToken) throws Exception {
        Uri uri = Uri.parse("content://"
            + AccountSessionRoutingPolicy.authority(application.getPackageName()));
        Bundle extras = null;
        if (AccountTokenPolicy.validToken(rejectedToken)) {
            extras = new Bundle();
            extras.putString(AccountSessionRoutingPolicy.KEY_REJECTED_ACCESS_TOKEN, rejectedToken);
        }
        Bundle reply = application.getContentResolver().call(
            uri, AccountSessionRoutingPolicy.METHOD_ANONYMOUS_ACCESS_TOKEN, null, extras);
        if (reply == null) throw new IllegalStateException("anonymous account unavailable");
        return AccountSessionRoutingPolicy.anonymousTokenFromReply(
            reply.getString(AccountSessionRoutingPolicy.KEY_STATE),
            reply.getString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN));
    }

    /**
     * Registers the identity with the backend unless a session is already saved, as the app does on first launch.
     *
     * <p>Unlike {@link #accessToken} an expired saved session is enough: registration has happened, and the first request that needs a token renews it. Without this every app start after the access token expired would sign in again.
     */
    void ensureRegistered() throws Exception {
        synchronized (LOCK) {
            if (sessions.load() == null) accessToken();
        }
    }

    /**
     * The anonymous subject already on this device, or an empty string when there is none.
     *
     * <p>Read-only on purpose: the settings screen shows this, and merely looking at that screen
     * must not be what creates the identity. {@link #ensureRegistered} creates it on first launch,
     * and {@link #accessToken} on the first request that needs it if that launch did not.
     */
    String savedSubject() {
        try {
            String saved = credentials.load();
            if (saved == null) return "";
            String subject = BackendAccount.optionalStringField(new JSONObject(saved).opt("subject"), "");
            return subject.matches("msime-[a-z0-9]{16}") ? subject : "";
        } catch (Exception error) {
            return "";
        }
    }

    /**
     * Create the device's anonymous identity if it has none, without reaching the network.
     *
     * <p>The identity is generated here, not issued by the backend -- a subject and a secret, both
     * random, both device-local. So there is no reason to make it wait for a server that may be
     * refusing requests: it exists as soon as anything asks for it.
     */
    String ensureSubject() {
        synchronized (LOCK) {
            try {
                return loadOrCreateIdentity().getString("subject");
            } catch (Exception | LinkageError error) {
                return "";
            }
        }
    }

    private JSONObject loadOrCreateIdentity() throws Exception {
        String saved = credentials.load();
        if (saved != null) {
            JSONObject identity = new JSONObject(saved);
            if (BackendAccount.optionalStringField(identity.opt("subject"), "").matches("msime-[a-z0-9]{16}")
                    && BackendAccount.optionalStringField(identity.opt("secret"), "").matches("[a-z0-9]{48}")) return identity;
        }
        JSONObject identity = new JSONObject().put("subject", "msime-" + random(16))
            .put("secret", random(48));
        credentials.save(identity.toString());
        return identity;
    }

    private static String random(int length) {
        final char[] alphabet = "abcdefghijklmnopqrstuvwxyz0123456789".toCharArray();
        StringBuilder value = new StringBuilder(length);
        for (int index = 0; index < length; index++) value.append(alphabet[RANDOM.nextInt(alphabet.length)]);
        return value.toString();
    }

    private static String tokenFromSession(String encoded) {
        try {
            JSONObject session = new JSONObject(encoded);
            long expiry = AccountTokenPolicy.strictLong(session.opt("expires_at_unix_ms"), 0);
            if (!AccountTokenPolicy.validExpiry(expiry, System.currentTimeMillis())) return null;
            String token = BackendAccount.optionalStringField(
                session.getJSONObject("tokens").opt("access_token"), "");
            return AccountTokenPolicy.validToken(token) ? token : null;
        } catch (Exception ignored) { return null; }
    }

    private static JSONObject request(String method, String path, JSONObject body, String token) throws Exception {
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
            if (status == 429) {
                // 服务端给了重试时间就按它来；没给就退一分钟，别把这件事变成一个忙等的循环。
                long seconds = KeyboardGeometry.bounded(
                    connection.getHeaderFieldInt("Retry-After", 60), 1, 3600);
                synchronized (LOCK) {
                    nextAttemptAtMillis = System.currentTimeMillis() + seconds * 1000L;
                }
                throw new RateLimited(seconds * 1000L);
            }
            if (status != 200) throw new IllegalStateException(
                "anonymous account unavailable: HTTP " + status);
            try (InputStream input = connection.getInputStream()) {
                byte[] response = HttpBodyPolicy.readBounded(input, MAX_RESPONSE_BYTES);
                if (response == null) throw new IllegalStateException("anonymous account unavailable");
                return new JSONObject(new String(response, StandardCharsets.UTF_8));
            }
        } finally { if (connection != null) connection.disconnect(); }
    }
}
