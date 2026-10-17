package app.msime.android;

import java.io.IOException;
import java.net.Proxy;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.URL;
import java.net.URLConnection;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * AI 辅助接口地址的传输策略：什么样的地址可以收到用户的 API Token 和输入内容。
 *
 * <p>https 不限主机；http 只能指向本机或局域网，公网主机必须走 https，API Token 不能明文经过互联网。规则只在 Rust（crates/client-providers/src/ai/endpoint.rs），这里经 {@code msime_client_ai_endpoint_check} 询问，不再自己判断；设备冒烟经 JNI 核对 shared/contracts/ai-endpoint/cases.json 里的写法。
 *
 * <p>Rust 放行的地址还要能构造出发请求用的 {@link URI}，构造不出的（路径里有空格这类 java.net.URI 不收的写法）在这里就按不合规处理，设置页也不收，免得存下一个输入法发不出去的地址。国际化域名 java.net.URI 认不出主机，主机和端口换成 Rust 来源键里的 punycode 写法，请求发往的正是保存 Token 的那个来源。
 *
 * <p>云候选（Google）不走这里，仍然只接受 https；见 {@link OnlineCandidatePolicy#validURL}。
 */
public final class AiEndpointPolicy {
    /** 接口地址能接受的最大 UTF-8 字节数，与 client-providers 和设置页一致。 */
    public static final int MAX_ENDPOINT_BYTES = 2048;
    /** 公网 http 地址被拒绝时给用户看的说明。 */
    public static final String CLEARTEXT_REASON = "http 只能用于本机或局域网地址（如 192.168.x.x、localhost、*.local），公网服务请用 https，以免 API Token 明文经过互联网";

    public enum Result {
        ALLOWED,
        /** http 地址指向的不是本机或局域网主机。 */
        CLEARTEXT_PUBLIC,
        /** 不是带主机的完整 http(s) 地址，或带了用户名、密码、# 片段、控制字符、反斜杠，或 http 的主机不是规范写法。 */
        INVALID
    }

    /** Rust 对一个地址的结论：result，和保存 Token 用的来源键（不能用时为 null）。 */
    public static final class Verdict {
        static final Verdict INVALID = new Verdict(Result.INVALID, null);

        final Result result;
        final String origin;

        Verdict(Result result, String origin) {
            this.result = result;
            this.origin = result == Result.ALLOWED ? origin : null;
        }
    }

    /** 问 Rust 的那一步。应用里总是经 JNI；JVM 冒烟加载不了 libmsime_android.so，换成按用例作答的实现。 */
    interface Checker {
        Verdict check(String endpoint);
    }

    private static volatile Checker checker;

    private AiEndpointPolicy() {}

    /** 换掉问 Rust 的那一步，只给不能加载 JNI 的 JVM 冒烟用；传 null 恢复 JNI。 */
    static void useChecker(Checker replacement) {
        checker = replacement;
    }

    public static Result check(String endpoint) {
        return verdict(endpoint).result;
    }

    public static boolean allowed(String endpoint) {
        return check(endpoint) == Result.ALLOWED;
    }

    /** 发请求用的地址（去掉首尾空白后解析，主机总是 ASCII），不通过时为 null。 */
    public static URI uri(String endpoint) {
        Verdict verdict = rustVerdict(endpoint);
        return verdict.result == Result.ALLOWED ? sendable(endpoint, verdict.origin) : null;
    }

    /** 保存 Token 用的来源键 `scheme://host:port`，由 Rust 算出，与设置页、iOS 和桌面端逐字相同；地址不通过时为 null。 */
    public static String credentialOrigin(String endpoint) {
        return verdict(endpoint).origin;
    }

    /** 本机、局域网的 http 地址必须直连：系统或 Wi-Fi 设置的代理会收到明文 Token，而代理可能在公网上。https 地址照旧按系统设置走。 */
    public static boolean bypassesProxy(URL target) {
        return "http".equalsIgnoreCase(target.getProtocol());
    }

    /** 打开 AI 请求的连接；http 地址绕过代理，见 {@link #bypassesProxy(URL)}。 */
    public static URLConnection open(URL target) throws IOException {
        return bypassesProxy(target) ? target.openConnection(Proxy.NO_PROXY) : target.openConnection();
    }

    /** Rust 的结论，再加上一条：放行的地址必须构造得出 {@link #uri(String)}，否则按不合规处理。 */
    private static Verdict verdict(String endpoint) {
        Verdict verdict = rustVerdict(endpoint);
        return verdict.result == Result.ALLOWED && sendable(endpoint, verdict.origin) == null ? Verdict.INVALID : verdict;
    }

    private static Verdict rustVerdict(String endpoint) {
        String value = endpoint == null ? "" : endpoint;
        Checker current = checker;
        return current != null ? current.check(value) : Native.check(value);
    }

    /** Rust 已放行的地址构造成 {@link URI}。java.net.URI 认不出主机时（国际化域名会被当成 registry authority，主机为 null），把 `://` 之后到第一个 `/` 或 `?` 为止的主机部分换成 Rust 来源键里的主机和端口（默认端口不写）；Rust 已拒收用户名、密码、片段和反斜杠，这一段只会是主机和端口。仍然构造不出时为 null。 */
    private static URI sendable(String endpoint, String origin) {
        String value = TextPolicy.trimmed(endpoint);
        try {
            URI uri = new URI(value);
            if (asciiHost(uri)) return uri;
            int separator = value.indexOf("://");
            String authority = origin == null ? null : originAuthority(origin);
            if (separator < 0 || authority == null) return null;
            int end = separator + 3;
            while (end < value.length() && value.charAt(end) != '/' && value.charAt(end) != '?') end++;
            URI rebuilt = new URI(authority + value.substring(end));
            return asciiHost(rebuilt) ? rebuilt : null;
        } catch (URISyntaxException error) {
            return null;
        }
    }

    private static boolean asciiHost(URI uri) {
        String host = uri.getHost();
        if (host == null || host.isEmpty()) return false;
        for (int index = 0; index < host.length(); index++) if (host.charAt(index) > 0x7f) return false;
        return true;
    }

    /** 来源键 `scheme://host:port` 去掉默认端口（https 的 443、http 的 80），读不懂时为 null。 */
    private static String originAuthority(String origin) {
        int separator = origin.indexOf("://");
        int colon = origin.lastIndexOf(':');
        if (separator < 0 || colon <= separator) return null;
        String scheme = origin.substring(0, separator);
        String port = origin.substring(colon + 1);
        boolean defaultPort = ("https".equals(scheme) && "443".equals(port)) || ("http".equals(scheme) && "80".equals(port));
        return defaultPort ? origin.substring(0, colon) : origin;
    }

    /** 经 JNI 问 Rust。首尾空格交给 Rust 去掉，制表符、换行这类控制字符由 Rust 拒绝，与原来先查原文再去空白相同。 */
    private static final class Native {
        static Verdict check(String endpoint) {
            try {
                JSONObject envelope = new JSONObject(NativeClient.aiEndpointCheck(endpoint));
                JSONObject value = envelope.optBoolean("ok") ? envelope.optJSONObject("value") : null;
                if (value == null || !value.has("problem")) return Verdict.INVALID;
                Object problem = value.get("problem");
                if (problem == JSONObject.NULL) {
                    Object origin = value.opt("origin");
                    return origin instanceof String ? new Verdict(Result.ALLOWED, (String) origin) : Verdict.INVALID;
                }
                return "cleartext_public".equals(problem) ? new Verdict(Result.CLEARTEXT_PUBLIC, null) : Verdict.INVALID;
            } catch (JSONException error) {
                return Verdict.INVALID;
            }
        }
    }
}
