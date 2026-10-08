package app.msime.android;

import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** AI 接口地址策略与 client-core 跑同一组用例：shared/contracts/ai-endpoint/cases.json 的每一条都要得出相同的结论和来源键。 */
public final class AiEndpointPolicySmoke {
    // 冒烟只有 android.jar 里抛 Stub! 的 org.json，用例文件每条一行，按固定的三个字段逐行匹配。
    private static final Pattern CASE = Pattern.compile(
        "\\{\\s*\"endpoint\":\\s*\"((?:[^\"\\\\]|\\\\.)*)\",\\s*\"result\":\\s*\"([a-z_]+)\",\\s*\"origin\":\\s*(?:null|\"((?:[^\"\\\\]|\\\\.)*)\")\\s*\\}");

    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] arguments) throws Exception {
        String contract = new String(Files.readAllBytes(contractPath()), StandardCharsets.UTF_8);
        Matcher matcher = CASE.matcher(contract);
        int cases = 0;
        while (matcher.find()) {
            String endpoint = unescape(matcher.group(1));
            String expected = matcher.group(2);
            String origin = matcher.group(3) == null ? null : unescape(matcher.group(3));
            String actual = switch (AiEndpointPolicy.check(endpoint)) {
                case ALLOWED -> "allowed";
                case CLEARTEXT_PUBLIC -> "cleartext_public";
                case INVALID -> "invalid";
            };
            check(expected.equals(actual), endpoint + ": expected " + expected + ", got " + actual);
            String actualOrigin = AiEndpointPolicy.credentialOrigin(endpoint);
            check(origin == null ? actualOrigin == null : origin.equals(actualOrigin),
                endpoint + ": expected origin " + origin + ", got " + actualOrigin);
            cases++;
        }
        int declared = contract.split("\"endpoint\"", -1).length - 1;
        check(cases > 40 && cases == declared, "matched " + cases + " of " + declared + " contract cases");

        // 非规范的 IPv6 写法得出与 WHATWG URL 相同的来源键，设置页存下的 Token 在键盘这边找得到。
        check("http://[fd12::1]:1234".equals(AiEndpointPolicy.credentialOrigin("http://[FD12:0:0::1]:1234/v1")),
            "IPv6 origin must use the RFC 5952 form");
        // 写法不规范的 IPv4 不做猜测，按公网域名拒绝。
        check(AiEndpointPolicy.check("http://010.0.0.1/v1") == AiEndpointPolicy.Result.CLEARTEXT_PUBLIC,
            "leading-zero IPv4 must not be read as a private address");

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
        System.out.println("Android AI endpoint policy passed " + cases + " contract cases");
    }

    /** 从当前目录向上找仓库里的用例文件，check-host.sh 可以从任何目录运行。 */
    private static Path contractPath() {
        for (Path directory = Paths.get(System.getProperty("user.dir")).toAbsolutePath(); directory != null;
                directory = directory.getParent()) {
            Path candidate = directory.resolve("shared/contracts/ai-endpoint/cases.json");
            if (Files.isRegularFile(candidate)) return candidate;
        }
        throw new AssertionError("shared/contracts/ai-endpoint/cases.json not found above the working directory");
    }

    private static String unescape(String value) {
        StringBuilder result = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char current = value.charAt(index);
            if (current != '\\') {
                result.append(current);
                continue;
            }
            char escaped = value.charAt(++index);
            switch (escaped) {
                case 'n' -> result.append('\n');
                case 't' -> result.append('\t');
                case 'r' -> result.append('\r');
                case 'u' -> {
                    result.append((char) Integer.parseInt(value.substring(index + 1, index + 5), 16));
                    index += 4;
                }
                default -> result.append(escaped);
            }
        }
        return result.toString();
    }
}
