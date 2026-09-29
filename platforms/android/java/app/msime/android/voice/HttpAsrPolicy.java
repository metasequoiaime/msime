package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.util.Locale;

/**
 * Whether a configured transcription provider can be used here, and what to send it.
 *
 * <p>The shared layer resolves the provider, endpoint, model and token from the settings document
 * and validates them before they reach this host, so nothing here re-derives a default. What is
 * left is the part that belongs to the transport: which providers this host can actually talk to,
 * and how the OpenAI-compatible multipart upload is assembled.
 *
 * <p>`doubao` is the streaming WebSocket protocol and is not implemented here yet. It is reported
 * as unsupported rather than failed, because the caller's answer to "unsupported" is to use the
 * platform recognizer — a user who configured Doubao still gets voice input, just not that one.
 */
public final class HttpAsrPolicy {
    /** Each of these is the same OpenAI-compatible `/audio/transcriptions` upload. */
    private static final String[] SUPPORTED = {
        "openai", "siliconflow", "groq", "everyapi", "mistral",
    };
    /** Anything beyond this is a runaway recording rather than a sentence. */
    public static final int MAX_AUDIO_BYTES = 24 * 1024 * 1024;

    private HttpAsrPolicy() {}

    public static boolean supported(String provider) {
        if (provider == null) return false;
        for (String candidate : SUPPORTED) {
            if (candidate.equals(provider)) return true;
        }
        return false;
    }

    /**
     * Whether this host can run the request as given.
     *
     * <p>The bounds mirror the shared validation rather than replacing it; what is genuinely this
     * host's to check is the scheme, because an `http://` endpoint would put the user's token on
     * the wire in clear text and this host is the one opening the connection.
     */
    public static boolean usable(String provider, String endpoint, String model, String token) {
        return supported(provider)
            && TextPolicy.validAuthority(endpoint, "https://", 2048)
            && model != null && !model.trim().isEmpty() && model.length() <= 512
            && !TextPolicy.hasControl(model)
            && token != null && !token.trim().isEmpty() && token.length() <= 16 * 1024
            && !TextPolicy.hasControl(token);
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
        StringBuilder head = new StringBuilder();
        appendField(head, boundary, "model", model);
        String trimmed = language == null ? "" : language.trim();
        if (!trimmed.isEmpty()) appendField(head, boundary, "language", isoLanguage(trimmed));
        head.append("--").append(boundary).append("\r\n")
            .append("Content-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\n")
            .append("Content-Type: audio/wav\r\n\r\n");
        byte[] prefix = head.toString().getBytes(StandardCharsets.UTF_8);
        byte[] suffix = ("\r\n--" + boundary + "--\r\n").getBytes(StandardCharsets.UTF_8);
        byte[] body = new byte[prefix.length + wav.length + suffix.length];
        System.arraycopy(prefix, 0, body, 0, prefix.length);
        System.arraycopy(wav, 0, body, prefix.length, wav.length);
        System.arraycopy(suffix, 0, body, prefix.length + wav.length, suffix.length);
        return body;
    }

    /**
     * The two-letter code these APIs expect, from the BCP 47 tag the rest of the client uses.
     *
     * <p>`zh-CN` and `zh-TW` are both `zh` to them; the script the user wants back is the client's
     * own 简繁 setting, not something the transcriber decides.
     */
    public static String isoLanguage(String language) {
        if (language == null) return "";
        String trimmed = language.trim();
        int separator = trimmed.indexOf('-');
        String primary = separator < 0 ? trimmed : trimmed.substring(0, separator);
        return primary.toLowerCase(Locale.ROOT);
    }

    private static void appendField(StringBuilder body, String boundary, String name, String value) {
        body.append("--").append(boundary).append("\r\n")
            .append("Content-Disposition: form-data; name=\"").append(name).append("\"\r\n\r\n")
            .append(value).append("\r\n");
    }

}
