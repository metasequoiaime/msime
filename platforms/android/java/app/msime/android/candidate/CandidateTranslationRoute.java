package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Objects;

/**
 * 键盘这一次用哪个在线服务给候选词补译文，以及用到的凭据。由 {@link CandidateTranslationPolicy#route} 从共享偏好算出，不可变：偏好变了就换一个新对象，键盘随之重建用户服务的缓存，工作线程上不会读到改了一半的凭据。
 *
 * <p>带着凭据，所以不用 record：record 自动生成的 {@code toString} 会把密钥打进日志。这里的 {@link #toString} 只说是哪个服务，{@link #scope} 用凭据的 SHA-256 前缀区分缓存，与 iOS 的 {@code cacheScope}、鸿蒙的 {@code providerScope} 作用相同。
 */
public final class CandidateTranslationRoute {
    /** 腾讯云机器翻译没填地域时共享层用的地域，与偏好的默认值相同。 */
    public static final String DEFAULT_TENCENT_REGION = "ap-guangzhou";

    public enum Provider {
        /** 没有选，或者选中的服务凭据不全：不联网翻译。 */
        NONE,
        /** 水杉账号（api.msime.app），走 {@link BackendTranslationClient}。 */
        ACCOUNT,
        NIUTRANS,
        CUSTOM,
        TENCENT,
        /** 复用现有 AI 助手配置的 Chat Completions 翻译。 */
        AI
    }

    public static final CandidateTranslationRoute NONE =
        new CandidateTranslationRoute(Provider.NONE, "", "", "", "", "", "");
    public static final CandidateTranslationRoute ACCOUNT =
        new CandidateTranslationRoute(Provider.ACCOUNT, "", "", "", "", "", "");

    private final Provider provider;
    private final String appId;
    private final String apiKey;
    private final String endpoint;
    private final String secretId;
    private final String secretKey;
    private final String region;

    private CandidateTranslationRoute(Provider provider, String appId, String apiKey, String endpoint,
            String secretId, String secretKey, String region) {
        this.provider = provider;
        this.appId = appId;
        this.apiKey = apiKey;
        this.endpoint = endpoint;
        this.secretId = secretId;
        this.secretKey = secretKey;
        this.region = region;
    }

    static CandidateTranslationRoute niutrans(String appId, String apiKey) {
        return new CandidateTranslationRoute(Provider.NIUTRANS, appId, apiKey, "", "", "", "");
    }

    static CandidateTranslationRoute custom(String endpoint, String apiKey) {
        return new CandidateTranslationRoute(Provider.CUSTOM, "", apiKey, endpoint, "", "", "");
    }

    static CandidateTranslationRoute tencent(String secretId, String secretKey, String region) {
        return new CandidateTranslationRoute(Provider.TENCENT, "", "", "", secretId, secretKey,
            region.isEmpty() ? DEFAULT_TENCENT_REGION : region);
    }

    static CandidateTranslationRoute ai(String provider, String endpoint, String model, String token) {
        return new CandidateTranslationRoute(Provider.AI, provider, token, endpoint, "", "", model);
    }

    public Provider provider() { return provider; }

    /** 是不是键盘直接请求的用户服务（含复用 AI 助手配置），这些路径不经过水杉账号。 */
    public boolean userService() {
        return provider == Provider.NIUTRANS || provider == Provider.CUSTOM
            || provider == Provider.TENCENT || provider == Provider.AI;
    }

    /** 小牛翻译的 App ID。 */
    public String appId() { return appId; }

    /** 小牛翻译或自定义服务的 API Key；自定义服务可以为空（DeepLX 允许不带密钥）。 */
    public String apiKey() { return apiKey; }

    /** 自定义服务的接口地址。 */
    public String endpoint() { return endpoint; }

    public String secretId() { return secretId; }

    public String secretKey() { return secretKey; }

    /** 腾讯云的地域，没填时是 {@link #DEFAULT_TENCENT_REGION}。 */
    public String region() { return region; }

    /** AI 服务商名称；只有 {@link Provider#AI} 路由使用。 */
    public String aiProvider() { return appId; }

    /** AI 模型名称；只有 {@link Provider#AI} 路由使用。 */
    public String aiModel() { return region; }

    /** AI 请求令牌；只在构造描述器时读取，不进入日志。 */
    public String aiToken() { return apiKey; }

    /** 缓存归属：服务加凭据的 SHA-256 前 16 位十六进制。换服务或换凭据后它就变，旧服务的译文不会留到新服务下。 */
    public String scope() {
        if (!userService()) return provider.name();
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException(impossible);
        }
        for (String field : new String[] {appId, apiKey, endpoint, secretId, secretKey, region}) {
            digest.update(field.getBytes(StandardCharsets.UTF_8));
            digest.update((byte) 0);
        }
        return provider.name() + "|" + DigestPolicy.hex(digest.digest()).substring(0, 16);
    }

    @Override public boolean equals(Object other) {
        if (this == other) return true;
        if (!(other instanceof CandidateTranslationRoute)) return false;
        CandidateTranslationRoute route = (CandidateTranslationRoute) other;
        return provider == route.provider && appId.equals(route.appId) && apiKey.equals(route.apiKey)
            && endpoint.equals(route.endpoint) && secretId.equals(route.secretId)
            && secretKey.equals(route.secretKey) && region.equals(route.region);
    }

    @Override public int hashCode() {
        return Objects.hash(provider, appId, apiKey, endpoint, secretId, secretKey, region);
    }

    /** 只写服务名，不写任何凭据。 */
    @Override public String toString() {
        return "CandidateTranslationRoute[" + provider + "]";
    }
}
