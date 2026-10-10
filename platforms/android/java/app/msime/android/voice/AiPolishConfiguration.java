package app.msime.android;

import java.net.URI;
import java.util.Objects;

/** Immutable, display-safe AI text-polish configuration. Secrets are never exposed by toString(). */
public final class AiPolishConfiguration {
    /** 自定义接口地址能接受的最大 UTF-8 字节数，规则见 {@link AiEndpointPolicy}。 */
    public static final int MAX_ENDPOINT_LENGTH = AiEndpointPolicy.MAX_ENDPOINT_BYTES;
    public static final int MAXIMUM_TEXT_CODE_POINTS = 10_000;
    public static final int MAXIMUM_RESPONSE_BYTES = 1024 * 1024;
    public static final String DEFAULT_PROMPT = "请润色以下文字，保持原意，只返回修改后的文字。";

    private final URI endpoint;
    private final String model;
    private final String prompt;
    private final String token;
    private final String credentialOrigin;

    public AiPolishConfiguration(String endpoint, String model, String prompt, String token) {
        this.endpoint = validatedEndpoint(endpoint);
        this.model = bounded(model, 512, "模型名称无效");
        this.prompt = bounded(prompt, 16 * 1024, "提示词无效");
        this.token = boundedOptional(token, 4096, "API Token 无效");
        this.credentialOrigin = AiEndpointPolicy.origin(this.endpoint);
    }

    public URI endpoint() { return endpoint; }
    public String model() { return model; }
    public String prompt() { return prompt; }
    String token() { return token; }
    public String credentialOrigin() { return credentialOrigin; }

    /** 给用户看的目标地址：真实的协议和主机，默认端口不写。 */
    public String destination() {
        String scheme = TextPolicy.lowercase(endpoint.getScheme());
        int port = endpoint.getPort();
        boolean defaultPort = port == -1 || port == ("https".equals(scheme) ? 443 : 80);
        return scheme + "://" + TextPolicy.lowercase(endpoint.getHost()) + (defaultPort ? "" : ":" + port);
    }

    public AiPolishConfiguration withPrompt(String replacement) {
        return new AiPolishConfiguration(endpoint.toString(), model, replacement, token);
    }

    /** The `ai_assistant` key holding the prompt the user selected with `prompt_id`: one of the three custom slots, the first when nothing else is named. */
    public static String promptSlotKey(String promptId) {
        if ("custom_2".equals(promptId) || "custom_3".equals(promptId)) return "prompt_" + promptId;
        return "prompt_custom_1";
    }

    public static String credentialOrigin(String endpoint) {
        return AiEndpointPolicy.origin(validatedEndpoint(endpoint));
    }

    public static boolean acceptableText(String text) {
        if (!TextPolicy.hasText(text) || !TextPolicy.validUnicode(text)) return false;
        return TextPolicy.withinCodePoints(text, MAXIMUM_TEXT_CODE_POINTS);
    }

    /** https 不限主机，http 只能指向本机或局域网；规则在 {@link AiEndpointPolicy}。 */
    private static URI validatedEndpoint(String value) {
        switch (AiEndpointPolicy.check(value)) {
            case ALLOWED:
                return AiEndpointPolicy.uri(value);
            case CLEARTEXT_PUBLIC:
                throw new IllegalArgumentException(AiEndpointPolicy.CLEARTEXT_REASON);
            default:
                throw new IllegalArgumentException("AI 接口地址无效");
        }
    }

    private static String bounded(String value, int maximum, String message) {
        String result = TextPolicy.trimmed(value);
        if (result.isEmpty() || TextPolicy.utf8Length(result) > maximum
                || TextPolicy.hasControlExceptWhitespace(result)
                || !TextPolicy.validUnicode(result)) throw new IllegalArgumentException(message);
        return result;
    }

    private static String boundedOptional(String value, int maximum, String message) {
        String result = TextPolicy.trimmed(value);
        if (TextPolicy.utf8Length(result) > maximum || TextPolicy.hasControl(result)
                || !TextPolicy.validUnicode(result))
            throw new IllegalArgumentException(message);
        return result;
    }

    @Override public boolean equals(Object other) {
        if (!(other instanceof AiPolishConfiguration configuration)) return false;
        return endpoint.equals(configuration.endpoint) && model.equals(configuration.model)
            && prompt.equals(configuration.prompt) && token.equals(configuration.token);
    }

    @Override public int hashCode() { return Objects.hash(endpoint, model, prompt, token); }

    @Override public String toString() {
        return "AiPolishConfiguration{" + destination() + ", model=" + model + "}";
    }
}
