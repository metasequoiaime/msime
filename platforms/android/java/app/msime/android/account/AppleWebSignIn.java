package app.msime.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Apple 网页登录的客户端部分（PKCE 式一次性授权码）。
 *
 * <p>安卓没有 Apple 的原生登录，只能在浏览器里走 Apple 的网页授权：本机生成 32 字节的 `code_verifier`，只把它的 S256 摘要交给服务端；Apple 回调到服务端后，服务端把一枚一次性 `grant` 经 `<applicationId>://auth/apple?grant=…` 交回本应用；本应用再用 grant 加上只在本机的 verifier 换会话。回调地址里没有 challenge id 也没有 id_token，伪造的回调因为没有对应的 verifier 换不到任何东西。
 *
 * <p>等待中的流程存在 `SharedPreferences("msime_auth_pending")`，只此一条、10 分钟有效，新流程覆盖旧流程。回调先检查形状（{@link #acceptableCallback}），形状不对的直接丢弃、不碰这条记录；形状对的只读出记录（{@link #peekPending}）去兑换，兑换成功或确定失败后才按 verifier 删掉它（{@link #clearPending(Context, String)}）。服务端以「grant 不认识、已用过或属于别的 challenge」拒绝时（{@link #keepPendingAfter}）记录保留，所以伪造的或旧标签页里的回调抢不走正在进行的登录；兑换由调用方串行进行，同一条记录不会被并发兑换两次。登录成功不触发任何同步。
 */
public final class AppleWebSignIn {
    /** Maximum length of the one-time web login grant accepted by the account endpoint. */
    public static final int MAX_GRANT_LENGTH = 128;
    static final String STORE = "msime_auth_pending";
    static final String KEY_VERIFIER = "verifier";
    static final String KEY_PURPOSE = "purpose";
    static final String KEY_CREATED_AT = "created_at";
    /** 等待中的流程最多保留 10 分钟；服务端的授权地址 5 分钟过期，grant 签发后 120 秒内必须兑换。 */
    static final long PENDING_MILLIS = 10 * 60_000L;
    /** 服务端只认这几个 `app`，与 `shared/contracts/editions.json` 的版本一一对应（full 版是不带后缀的包名）。 */
    public static final List<String> APPS = List.of("app.msime.android", "app.msime.android.pinyin",
        "app.msime.android.wubi", "app.msime.android.japanese", "app.msime.android.vietnamese",
        "app.msime.android.tibetan");
    private static final String AUTHORIZATION_ORIGIN = "https://appleid.apple.com/";
    private static final SecureRandom RANDOM = new SecureRandom();
    private static final Object PENDING_LOCK = new Object();

    /** 本机等着回调的那一次流程。 */
    public record Pending(String verifier, String purpose, long createdAt) {
        public boolean link() { return "link".equals(purpose); }
    }

    private AppleWebSignIn() {}

    /** 32 字节随机数的 base64url（无填充），43 个字符。 */
    public static String newVerifier(SecureRandom random) {
        byte[] bytes = new byte[32];
        random.nextBytes(bytes);
        return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
    }

    /** RFC 7636 的 S256：`base64url(SHA-256(ASCII(verifier)))`，无填充。 */
    public static String challenge(String verifier) {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256").digest(verifier.getBytes(StandardCharsets.US_ASCII));
            return Base64.getUrlEncoder().withoutPadding().encodeToString(digest);
        } catch (NoSuchAlgorithmException missing) {
            throw new IllegalStateException("SHA-256 unavailable", missing);
        }
    }

    /** 等待中的流程是否还能用：创建时间不在将来、且不超过 10 分钟。 */
    static boolean fresh(long createdAt, long now) {
        return createdAt > 0 && createdAt <= now && now - createdAt <= PENDING_MILLIS;
    }

    /** 服务端返回的授权地址必须是 Apple 自己的 HTTPS 页面，否则不打开。 */
    static boolean validAuthorizationUrl(String url) {
        if (url == null || !url.startsWith(AUTHORIZATION_ORIGIN) || url.length() > 4096) return false;
        for (int index = 0; index < url.length(); index++) {
            char c = url.charAt(index);
            if (c <= ' ' || c >= 0x7F) return false;
        }
        return true;
    }

    /** 回调里的 grant：服务端生成的 32 字节 base64url（43 个字符），只接受这个形状。 */
    public static boolean validGrant(String grant) {
        if (grant == null || grant.length() < 32 || grant.length() > MAX_GRANT_LENGTH) return false;
        for (int index = 0; index < grant.length(); index++) {
            char c = grant.charAt(index);
            boolean allowed = c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-' || c == '_';
            if (!allowed) return false;
        }
        return true;
    }

    /**
     * 开始一次流程：生成并保存 verifier，向服务端要 Apple 的授权地址。阻塞，不要在主线程调用。
     *
     * @param purpose `login`，或把 Apple 绑到已登录账号的 `link`（需要最近登录过的会话）
     * @return 要在浏览器里打开的授权地址
     */
    public static String start(Context context, String purpose) throws CloudApi.Failure {
        if (!"login".equals(purpose) && !"link".equals(purpose)) throw new IllegalArgumentException("invalid purpose");
        Context application = context.getApplicationContext();
        String app = application.getPackageName();
        if (!APPS.contains(app)) throw new CloudApi.Failure(400, "invalid_app", "unsupported package", 0);
        String verifier = newVerifier(RANDOM);
        store(application).edit().putString(KEY_VERIFIER, verifier).putString(KEY_PURPOSE, purpose)
            .putLong(KEY_CREATED_AT, System.currentTimeMillis()).commit();
        JSONObject body;
        try {
            body = new JSONObject().put("code_challenge", challenge(verifier)).put("code_challenge_method", "S256")
                .put("app", app).put("purpose", purpose);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        JSONObject response = new CloudApi(application).json("POST", "/v1/auth/apple/web", body,
            "link".equals(purpose) ? CloudApi.Auth.ACCOUNT : CloudApi.Auth.NONE);
        Object url = response.opt("authorization_url");
        if (!(url instanceof String value) || !validAuthorizationUrl(value)) {
            clearPending(application);
            throw new CloudApi.Failure(500, "invalid_response", "authorization_url", 0);
        }
        return value;
    }

    /** 回调里 `error` 的形状：服务端只会给短的小写下划线代码。 */
    private static final int MAX_ERROR_LENGTH = 64;

    /**
     * 回调的形状对不对：`grant` 与 `error` 恰好有一个非空；grant 要过 {@link #validGrant}，error 要是不超过 64 个字符的 `[a-z_]` 代码。形状不对的回调不碰等待中的流程，直接丢弃。
     */
    public static boolean acceptableCallback(String grant, String error) {
        boolean hasGrant = grant != null && !grant.isEmpty();
        boolean hasError = error != null && !error.isEmpty();
        if (hasGrant == hasError) return false;
        if (hasGrant) return validGrant(grant);
        if (error.length() > MAX_ERROR_LENGTH) return false;
        for (int index = 0; index < error.length(); index++) {
            char c = error.charAt(index);
            if (!(c >= 'a' && c <= 'z' || c == '_')) return false;
        }
        return true;
    }

    /** 兑换失败后要不要保留等待中的流程：400 / 401 / 404 表示这个 grant 不认识、已用过或属于别的 challenge，真正的回调可能还在路上。 */
    public static boolean keepPendingAfter(int status) {
        return status == 400 || status == 401 || status == 404;
    }

    /** 读出等待中的流程但不删除；没有、或已过期时返回 null（过期的顺手删掉）。 */
    public static Pending peekPending(Context context) {
        return peekPending(store(context.getApplicationContext()), System.currentTimeMillis());
    }

    /** 只在保存的 verifier 仍是 `verifier` 时删掉等待中的流程；期间开始了新流程就不动它。 */
    public static void clearPending(Context context, String verifier) {
        clearPending(store(context.getApplicationContext()), verifier);
    }

    /** 放弃等待中的流程，例如用户关掉了登录面板。 */
    public static void clearPending(Context context) {
        store(context.getApplicationContext()).edit().clear().commit();
    }

    static Pending peekPending(SharedPreferences store, long now) {
        synchronized (PENDING_LOCK) {
            String verifier = store.getString(KEY_VERIFIER, null);
            String purpose = store.getString(KEY_PURPOSE, null);
            long createdAt = store.getLong(KEY_CREATED_AT, 0L);
            if (verifier == null || purpose == null) return null;
            if (!fresh(createdAt, now)) {
                store.edit().clear().commit();
                return null;
            }
            return new Pending(verifier, purpose, createdAt);
        }
    }

    static void clearPending(SharedPreferences store, String verifier) {
        synchronized (PENDING_LOCK) {
            if (verifier != null && verifier.equals(store.getString(KEY_VERIFIER, null))) store.edit().clear().commit();
        }
    }

    /**
     * 用回调里的 grant 和保存的 verifier 换会话并保存。阻塞，不要在主线程调用。
     *
     * @param userAgent {@link BackendAccount#loginUserAgent} 生成的详细 User-Agent
     */
    public static void complete(Context context, Pending pending, String grant, String userAgent) throws Exception {
        if (pending == null || !validGrant(grant)) throw new IllegalArgumentException("invalid grant");
        new BackendAccount(context.getApplicationContext())
            .loginWithAppleGrant(grant, pending.verifier(), pending.link(), userAgent);
    }

    private static SharedPreferences store(Context context) {
        return context.getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }
}
