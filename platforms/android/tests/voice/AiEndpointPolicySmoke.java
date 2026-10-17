package app.msime.android;

import java.net.URI;
import java.net.URL;

/** AI 接口地址策略的宿主一侧：Rust 的结论原样交给调用方，系统解析不了的地址不发，润色配置、模型目录和 AI 候选都经同一个结论。结论本身由 Rust 和设备冒烟核对，这里按 {@link AiEndpointContract} 的表作答。 */
public final class AiEndpointPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] arguments) throws Exception {
        int cases = AiEndpointContract.install();
        // 结论和来源键都取自 Rust，不在 Java 里改写；「不能用」的地址没有来源键。
        check(AiEndpointPolicy.check("HTTP://LOCALHOST:1234/v1") == AiEndpointPolicy.Result.ALLOWED
                && "http://localhost:1234".equals(AiEndpointPolicy.credentialOrigin("HTTP://LOCALHOST:1234/v1")),
            "an allowed endpoint carries Rust's origin");
        check(AiEndpointPolicy.check("http://8.8.8.8/v1/chat/completions") == AiEndpointPolicy.Result.CLEARTEXT_PUBLIC
                && AiEndpointPolicy.credentialOrigin("http://8.8.8.8/v1/chat/completions") == null,
            "a public http endpoint has no origin");
        // Rust 放行、java.net.URI 却解析不了的地址（路径里有空格）发不出去，所以整体按不合规处理：设置页（allowed）不收，也没有来源键，不会存下一个输入法发不出去的地址。
        check(AiEndpointPolicy.check("https://api.example.com/v1 chat") == AiEndpointPolicy.Result.INVALID
                && !AiEndpointPolicy.allowed("https://api.example.com/v1 chat")
                && AiEndpointPolicy.credentialOrigin("https://api.example.com/v1 chat") == null
                && AiEndpointPolicy.uri("https://api.example.com/v1 chat") == null,
            "an endpoint java.net.URI cannot parse is refused everywhere");
        // 国际化域名：java.net.URI 认不出主机，请求地址的主机换成 Rust 来源键里的 punycode，路径、端口和查询串照旧。设置页放行的，润色和 AI 候选也都发得出去。
        String idn = "https://例子.测试/v1/chat/completions";
        URI idnTarget = AiEndpointPolicy.uri(idn);
        check(AiEndpointPolicy.allowed(idn) && idnTarget != null
                && "https://xn--fsqu00a.xn--0zwm56d/v1/chat/completions".equals(idnTarget.toString()),
            "an internationalized host is sent as punycode");
        URI idnPort = AiEndpointPolicy.uri("https://例子.测试:8443/v1/chat/completions?x=1");
        check(idnPort != null && "https://xn--fsqu00a.xn--0zwm56d:8443/v1/chat/completions?x=1".equals(idnPort.toString()),
            "an internationalized host keeps its port and query");
        AiPolishConfiguration idnPolish = new AiPolishConfiguration(idn, "m", "p", "token");
        check("https://xn--fsqu00a.xn--0zwm56d".equals(idnPolish.destination())
                && "https://xn--fsqu00a.xn--0zwm56d:443".equals(idnPolish.credentialOrigin())
                && "https://xn--fsqu00a.xn--0zwm56d:443".equals(idnPolish.withPrompt("q").credentialOrigin()),
            "AI polish accepts an internationalized host and keeps Rust's origin");
        check(AiEndpointPolicy.allowed(AiPolishModelCatalog.modelsUri(idnPolish.endpoint()).toString()),
            "the model directory of an internationalized host passes the policy");
        // OnlineCandidateTransport.ai 先查 validAiURL，再用 uri(...).toURL() 连接；它收 JSONObject，JVM 冒烟里构造不了，这里核对它用到的这两步。
        check(OnlineCandidatePolicy.validAiURL(idn)
                && "xn--fsqu00a.xn--0zwm56d".equals(AiEndpointPolicy.uri(idn).toURL().getHost()),
            "AI candidates connect to the punycode host");
        check(AiEndpointPolicy.uri(null) == null && !AiEndpointPolicy.allowed(""), "a missing endpoint is refused");

        AiPolishConfiguration local = new AiPolishConfiguration(
            "http://192.168.1.20:1234/v1/chat/completions", "local-model", "prompt", "");
        check("http://192.168.1.20:1234".equals(local.destination()), "destination must keep the http scheme");
        check("http://192.168.1.20:1234".equals(local.credentialOrigin()), "local origin");
        check("http://localhost".equals(new AiPolishConfiguration(
            "http://localhost/v1", "m", "p", "").destination()), "default http port is not printed");
        try {
            new AiPolishConfiguration("http://api.example.com/v1/chat/completions", "m", "p", "token");
            throw new AssertionError("public http endpoint must be refused");
        } catch (IllegalArgumentException expected) {
            check(AiEndpointPolicy.CLEARTEXT_REASON.equals(expected.getMessage()),
                "public http refusal must explain why");
        }

        URI models = AiPolishModelCatalog.modelsUri(local.endpoint());
        check("http://192.168.1.20:1234/v1/models".equals(models.toASCIIString()),
            "local model directory keeps the http origin");
        check(AiEndpointPolicy.allowed(models.toString()), "local model directory passes the policy");

        check(OnlineCandidatePolicy.validAiURL("http://192.168.1.20:1234/v1/chat/completions"),
            "AI candidates may use a LAN http endpoint");
        check(!OnlineCandidatePolicy.validAiURL("http://8.8.8.8/v1/chat/completions"),
            "AI candidates must not use a public http endpoint");
        check(!OnlineCandidatePolicy.validURL(new URL("http://192.168.1.20:1234/translate")),
            "cloud candidates stay https-only");
        // 局域网的明文请求不能经系统代理转出去；https 照旧遵守系统代理设置。
        check(AiEndpointPolicy.bypassesProxy(new URL("http://192.168.1.20:1234/v1/chat/completions")),
            "LAN http requests must connect directly");
        check(!AiEndpointPolicy.bypassesProxy(new URL("https://api.example.com/v1/chat/completions")),
            "https requests keep the system proxy");
        System.out.println("Android AI endpoint policy passed with " + cases + " contract answers");
    }
}
