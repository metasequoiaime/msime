package app.msime.android;

import java.net.URI;
import java.util.Map;

/** Anthropic polish requests use native headers while compatible providers keep Bearer auth. */
public final class AiPolishHttpTransportSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] arguments) throws Exception {
        Map<String, String> anthropic = AiPolishHttpTransport.authenticationHeaders(
            new URI("https://api.anthropic.com/v1/chat/completions"), "fixture");
        check("fixture".equals(anthropic.get("x-api-key")),
            "Anthropic polish requests must send x-api-key");
        check("2023-06-01".equals(anthropic.get("anthropic-version")),
            "Anthropic polish requests must send the API version");
        check(!anthropic.containsKey("Authorization"),
            "Anthropic polish requests must not send a Bearer header");

        Map<String, String> compatible = AiPolishHttpTransport.authenticationHeaders(
            new URI("https://api.everyapi.ai/v1/chat/completions"), "fixture");
        check("Bearer fixture".equals(compatible.get("Authorization")),
            "Compatible polish requests must keep Bearer auth");
        check(!compatible.containsKey("x-api-key"),
            "Compatible polish requests must not send an Anthropic key header");

        check("polished".equals(JsonPolicy.strictStringOrEmpty("polished")),
            "AI polish accepts string content");
        check("".equals(JsonPolicy.strictStringOrEmpty(42)),
            "AI polish rejects numeric content instead of coercing it");
        check("".equals(JsonPolicy.strictStringOrEmpty(null)),
            "AI polish rejects null content instead of displaying it");
        // 宿主冒烟只有 android.jar 里抛 Stub! 的 org.json，请求体本身由 requestBody 按这条判断拼出，这里钉住判断。
        URI ark = new URI("https://ark.cn-beijing.volces.com/api/v3/chat/completions");
        check(AiPolishHttpTransport.disablesArkSeedThinking(ark, "doubao-seed-2-0-mini-260428"),
            "Ark Seed polish requests must turn thinking off");
        check(AiPolishHttpTransport.disablesArkSeedThinking(
                new URI("https://ark.cn-shanghai.volces.com/api/v3/chat/completions"), " Doubao-Seed-2-0-lite-260428"),
            "Every Ark region and model casing counts");
        check(!AiPolishHttpTransport.disablesArkSeedThinking(ark, "glm-5-3-flash-260828"),
            "Other Ark models keep their own thinking default");
        check(!AiPolishHttpTransport.disablesArkSeedThinking(ark, "ep-20261105123456-abcde"),
            "An endpoint ID does not say which model it serves");
        check(!AiPolishHttpTransport.disablesArkSeedThinking(
                new URI("https://volces.com.example/v1/chat/completions"), "doubao-seed-2-0-mini-260428"),
            "A look-alike host is not Ark");
        check(!AiPolishHttpTransport.disablesArkSeedThinking(
                new URI("https://api.everyapi.ai/v1/chat/completions"), "doubao-seed-2-0-mini-260428"),
            "Gateways serving the same model name are left alone");
        System.out.println("Android AI polish authentication passed");
    }
}
