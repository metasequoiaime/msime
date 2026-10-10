package app.msime.android;


/**
 * Whether a configured transcription provider can be used here, and what to send it.
 *
 * <p>共享层从设置文档解析出 provider、接口地址、模型和密钥，并在交给本宿主之前校验过，所以这里不再推导任何默认值。剩下的是属于传输层的部分：本宿主实际能发哪些请求格式，以及每种请求体怎么拼。共享层随配置给出请求格式（`requestFormat`），这里只按格式挑请求构造，不按 provider 名字判断。
 *
 * <p>`doubao` is the streaming WebSocket protocol and is not implemented here yet. It is reported
 * as unsupported rather than failed, because the caller's answer to "unsupported" is to use the
 * platform recognizer — a user who configured Doubao still gets voice input, just not that one.
 */
public final class HttpAsrPolicy {
    /** OpenAI 兼容的 `/audio/transcriptions` multipart 上传。 */
    public static final String MULTIPART = "multipart";
    /** Chat Completions 带 `input_audio` 的 JSON 请求（阿里云百炼），回答在 `choices[0].message.content`。 */
    public static final String CHAT_AUDIO = "chat_audio";
    /** 本宿主能发的整句上传格式；豆包的流式协议另走 {@link DoubaoAsrPolicy}。 */
    private static final String[] SUPPORTED = {MULTIPART, CHAT_AUDIO};
    /** Anything beyond this is a runaway recording rather than a sentence. */
    public static final int MAX_AUDIO_BYTES = 24 * 1024 * 1024;
    /** A provider response must fit the same transcript bound used by contribution uploads. */
    public static final int MAX_TRANSCRIPT = 2000;

    private HttpAsrPolicy() {}

    /** 共享层给出的请求格式是不是本宿主能发的整句上传。 */
    public static boolean supported(String requestFormat) {
        if (requestFormat == null) return false;
        for (String candidate : SUPPORTED) {
            if (candidate.equals(requestFormat)) return true;
        }
        return false;
    }

    /** A transcription response carries text; reject non-string JSON values before display. */
    static String strictText(Object value) {
        return AiProviderResponse.boundedText(value, MAX_TRANSCRIPT);
    }

    /**
     * Whether this host can run the request as given.
     *
     * <p>The bounds mirror the shared validation rather than replacing it; what is genuinely this
     * host's to check is the scheme, because an `http://` endpoint would put the user's token on
     * the wire in clear text and this host is the one opening the connection.
     */
    public static boolean usable(String requestFormat, String endpoint, String model, String token) {
        return supported(requestFormat)
            && TextPolicy.validAuthority(endpoint, "https://", AiPolishConfiguration.MAX_ENDPOINT_LENGTH)
            && model != null && !TextPolicy.trimmed(model).isEmpty() && TextPolicy.utf8Length(model) <= 512
            && !TextPolicy.hasControl(model) && TextPolicy.validUnicode(model)
            && token != null && !TextPolicy.trimmed(token).isEmpty()
            && TextPolicy.utf8Length(token) <= 16 * 1024
            && !TextPolicy.hasControl(token) && TextPolicy.validUnicode(token);
    }

    /** A boundary that cannot occur in the parts, derived from the request rather than random. */
    public static String boundary(String requestId) {
        StringBuilder safe = new StringBuilder("msime");
        String source = requestId == null ? "" : requestId;
        for (int index = 0; index < source.length() && safe.length() < 40; index++) {
            char value = source.charAt(index);
            if (value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z'
                    || value >= '0' && value <= '9' || value == '-') {
                safe.append(value);
            }
        }
        return safe.toString();
    }

    /**
     * The multipart body for one recording.
     *
     * <p>`language` is sent only when the caller has one worth sending: the providers treat an
     * absent language as "detect", and sending an empty value is not the same thing.
     */
    public static byte[] multipartBody(String boundary, String model, String language, byte[] wav) {
        String trimmed = TextPolicy.trimmed(language);
        StringBuilder head = new StringBuilder(128 + boundary.length()
            + VoiceTextPolicy.length(model) + trimmed.length());
        appendField(head, boundary, "model", model);
        if (!trimmed.isEmpty()) appendField(head, boundary, "language", isoLanguage(trimmed));
        head.append("--").append(boundary).append("\r\n")
            .append("Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n")
            .append("Content-Type: audio/wav\r\n\r\n");
        byte[] prefix = TextPolicy.utf8Bytes(head.toString());
        byte[] suffix = TextPolicy.utf8Bytes("\r\n--" + boundary + "--\r\n");
        byte[] body = new byte[prefix.length + wav.length + suffix.length];
        System.arraycopy(prefix, 0, body, 0, prefix.length);
        System.arraycopy(wav, 0, body, prefix.length, wav.length);
        System.arraycopy(suffix, 0, body, prefix.length + wav.length, suffix.length);
        return body;
    }

    /**
     * {@link #CHAT_AUDIO} 的请求体：唯一一条 user 消息，内容是录音的 Base64 数据 URL。不带语种，交给模型自动识别。
     */
    public static byte[] chatAudioBody(String model, byte[] wav) {
        String data = "data:audio/wav;base64," + java.util.Base64.getEncoder().encodeToString(wav);
        String body = "{\"model\":" + JsonPolicy.quote(model)
            + ",\"stream\":false,\"messages\":[{\"role\":\"user\",\"content\":[{\"type\":\"input_audio\","
            + "\"input_audio\":{\"data\":" + JsonPolicy.quote(data) + "}}]}]}";
        return TextPolicy.utf8Bytes(body);
    }

    /** 请求体的 Content-Type。 */
    public static String contentType(String requestFormat, String boundary) {
        return CHAT_AUDIO.equals(requestFormat)
            ? "application/json"
            : "multipart/form-data; boundary=" + boundary;
    }

    /**
     * The two-letter code these APIs expect, from the BCP 47 tag the rest of the client uses.
     *
     * <p>`zh-CN` and `zh-TW` are both `zh` to them; the script the user wants back is the client's
     * own 简繁 setting, not something the transcriber decides.
     */
    public static String isoLanguage(String language) {
        if (language == null) return "";
        String trimmed = TextPolicy.trimmed(language);
        int separator = trimmed.indexOf('-');
        int underscore = trimmed.indexOf('_');
        if (separator < 0 || (underscore >= 0 && underscore < separator)) {
            separator = underscore;
        }
        String primary = separator < 0 ? trimmed : trimmed.substring(0, separator);
        return TextPolicy.lowercase(primary);
    }

    private static void appendField(StringBuilder body, String boundary, String name, String value) {
        body.append("--").append(boundary).append("\r\n")
            .append("Content-Disposition: form-data; name=\"").append(name).append("\"\r\n\r\n")
            .append(value).append("\r\n");
    }

}
