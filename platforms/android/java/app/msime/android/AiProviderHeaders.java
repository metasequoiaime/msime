package app.msime.android;

import java.util.LinkedHashMap;
import java.util.Map;

/** Shared authentication headers for OpenAI-compatible and Anthropic AI endpoints. */
final class AiProviderHeaders {
    private AiProviderHeaders() {}

    static Map<String, String> forHost(String host, String token) {
        Map<String, String> headers = new LinkedHashMap<>(2);
        if (token == null || token.isEmpty()) return headers;
        if ("api.anthropic.com".equalsIgnoreCase(host)) {
            headers.put("x-api-key", token);
            headers.put("anthropic-version", "2023-06-01");
        } else {
            headers.put("Authorization", "Bearer " + token);
        }
        return headers;
    }
}
