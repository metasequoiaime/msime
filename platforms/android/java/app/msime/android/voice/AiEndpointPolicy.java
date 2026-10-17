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

    /** 通过检查的地址（去掉首尾空白后解析），不通过、或 Rust 放行而 {@link URI} 解析不了时为 null。 */
    public static URI uri(String endpoint) {
        if (!allowed(endpoint)) return null;
        try {
            URI uri = new URI(TextPolicy.trimmed(endpoint));
            return uri.getHost() == null || uri.getHost().isEmpty() ? null : uri;
        } catch (URISyntaxException error) {
            return null;
        }
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

    private static Verdict verdict(String endpoint) {
        String value = endpoint == null ? "" : endpoint;
        Checker current = checker;
        return current != null ? current.check(value) : Native.check(value);
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
