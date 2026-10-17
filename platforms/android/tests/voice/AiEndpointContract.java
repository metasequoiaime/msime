package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * JVM 冒烟里代替 JNI 回答 AI 接口地址的检查。地址规则只在 Rust，冒烟加载不了 libmsime_android.so，所以这里只按表作答：表里是 shared/contracts/ai-endpoint/cases.json 的全部用例（Rust 的 `shared_contract_cases` 逐条跑同一份），再加上几个冒烟各自用到、结论一眼可知的地址。表里没有的地址直接失败，免得冒烟悄悄依赖一个没人核对过的结论。
 *
 * <p>JNI 本身由设备冒烟 AiEndpointCheckDeviceSmoke 经真实的 libmsime_android.so 验证。
 */
public final class AiEndpointContract {
    // 冒烟只有 android.jar 里抛 Stub! 的 org.json，用例文件每条一行，按固定的三个字段逐行匹配。
    private static final Pattern CASE = Pattern.compile(
        "\\{\\s*\"endpoint\":\\s*\"((?:[^\"\\\\]|\\\\.)*)\",\\s*\"result\":\\s*\"([a-z_]+)\",\\s*\"origin\":\\s*(?:null|\"((?:[^\"\\\\]|\\\\.)*)\")\\s*\\}");
    private static final Map<String, AiEndpointPolicy.Verdict> ANSWERS = new HashMap<>();
    private static int contractCases;

    private AiEndpointContract() {}

    /** 装上按表作答的检查，返回 cases.json 里的用例数。 */
    public static synchronized int install() throws Exception {
        if (contractCases == 0) {
            String contract = new String(Files.readAllBytes(contractPath()), StandardCharsets.UTF_8);
            Matcher matcher = CASE.matcher(contract);
            while (matcher.find()) {
                answer(unescape(matcher.group(1)), matcher.group(2),
                    matcher.group(3) == null ? null : unescape(matcher.group(3)));
                contractCases++;
            }
            int declared = contract.split("\"endpoint\"", -1).length - 1;
            if (contractCases <= 40 || contractCases != declared)
                throw new AssertionError("matched " + contractCases + " of " + declared + " contract cases");
            // 冒烟用到的其余地址：普通的 https 主机都放行，来源键是小写主机加端口；超过 2048 字节的地址不收。
            answer("https://Fixture.Invalid/v1/chat/completions", "allowed", "https://fixture.invalid:443");
            answer("https://fixture.invalid/v1/chat/completions", "allowed", "https://fixture.invalid:443");
            answer("https://fixture.invalid/another/path", "allowed", "https://fixture.invalid:443");
            answer("https://fixture.invalid:444/v1/chat/completions", "allowed", "https://fixture.invalid:444");
            answer("https://fixture.invalid/" + "界".repeat(700), "invalid", null);
            answer("https://api.openai.com/v1/chat/completions", "allowed", "https://api.openai.com:443");
            answer("http://192.168.1.20:1234/v1/chat/completions", "allowed", "http://192.168.1.20:1234");
            answer("http://192.168.1.20:1234/v1/models", "allowed", "http://192.168.1.20:1234");
            answer("http://localhost/v1", "allowed", "http://localhost:80");
            answer("http://api.example.com/v1/chat/completions", "cleartext_public", null);
            answer("http://8.8.8.8/v1/chat/completions", "cleartext_public", null);
            // Rust 先把路径里的空格编码再判断，所以放行；java.net.URI 不收这样的原文。
            answer("https://api.example.com/v1 chat", "allowed", "https://api.example.com:443");
            // 国际化域名：Rust 按 punycode 放行（与设备冒烟 AiEndpointCheckDeviceSmoke 经真实 JNI 核对的那一条相同），换成 punycode 主机以后的地址结论和来源键不变。
            answer("https://例子.测试/v1/chat/completions", "allowed", "https://xn--fsqu00a.xn--0zwm56d:443");
            answer("https://例子.测试:8443/v1/chat/completions?x=1", "allowed", "https://xn--fsqu00a.xn--0zwm56d:8443");
            answer("https://xn--fsqu00a.xn--0zwm56d/v1/chat/completions", "allowed", "https://xn--fsqu00a.xn--0zwm56d:443");
            answer("https://xn--fsqu00a.xn--0zwm56d/v1/models", "allowed", "https://xn--fsqu00a.xn--0zwm56d:443");
            answer("", "invalid", null);
        }
        AiEndpointPolicy.useChecker(endpoint -> {
            AiEndpointPolicy.Verdict verdict = ANSWERS.get(endpoint);
            if (verdict == null) throw new AssertionError("no checked answer for AI endpoint " + endpoint);
            return verdict;
        });
        return contractCases;
    }

    private static void answer(String endpoint, String result, String origin) {
        AiEndpointPolicy.Result value = switch (result) {
            case "allowed" -> AiEndpointPolicy.Result.ALLOWED;
            case "cleartext_public" -> AiEndpointPolicy.Result.CLEARTEXT_PUBLIC;
            case "invalid" -> AiEndpointPolicy.Result.INVALID;
            default -> throw new AssertionError("unknown contract result " + result);
        };
        ANSWERS.put(endpoint, new AiEndpointPolicy.Verdict(value, origin));
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
