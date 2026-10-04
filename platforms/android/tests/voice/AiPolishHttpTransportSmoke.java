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

        check("polished".equals(AiPolishHttpTransport.strictContent("polished")),
            "AI polish accepts string content");
        check("".equals(AiPolishHttpTransport.strictContent(42)),
            "AI polish rejects numeric content instead of coercing it");
        check("".equals(AiPolishHttpTransport.strictContent(null)),
            "AI polish rejects null content instead of displaying it");
        System.out.println("Android AI polish authentication passed");
    }
}
