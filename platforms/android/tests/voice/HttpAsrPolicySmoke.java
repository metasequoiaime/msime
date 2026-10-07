import app.msime.android.HttpAsrPolicy;
import app.msime.android.WavAudio;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

/** Which providers this host can talk to, and the exact bytes it uploads. */
public final class HttpAsrPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static int read32(byte[] data, int offset) {
        return (data[offset] & 0xff) | (data[offset + 1] & 0xff) << 8
            | (data[offset + 2] & 0xff) << 16 | (data[offset + 3] & 0xff) << 24;
    }

    public static void main(String[] args) {
        for (String provider : new String[] {"openai", "siliconflow", "groq", "everyapi", "mistral"}) {
            check(HttpAsrPolicy.supported(provider), provider + " is an OpenAI-compatible upload");
        }
        // Doubao is the streaming WebSocket protocol. Reporting it unsupported is what sends the
        // caller to the platform recognizer, so a user who configured it still has voice input.
        check(!HttpAsrPolicy.supported("doubao"), "doubao is not an upload provider");
        check(!HttpAsrPolicy.supported("") && !HttpAsrPolicy.supported(null)
                && !HttpAsrPolicy.supported("whisper"),
            "an unset or unknown provider is unsupported");

        String endpoint = "https://api.openai.com/v1/audio/transcriptions";
        check(HttpAsrPolicy.usable("openai", endpoint, "whisper-1", "token"),
            "a complete configuration is usable");
        // This host opens the connection, so the scheme is its responsibility: http would put the
        // user's token on the wire in clear text.
        check(!HttpAsrPolicy.usable("openai", "http://api.openai.com/v1/audio/transcriptions",
            "whisper-1", "token"),
            "a plaintext endpoint is refused");
        check(!HttpAsrPolicy.usable("openai", "https:///audio/transcriptions",
            "whisper-1", "token"), "an endpoint without an authority is refused");
        check(!HttpAsrPolicy.usable("openai", endpoint, "", "token")
                && !HttpAsrPolicy.usable("openai", endpoint, "   ", "token"),
            "an empty model is refused");
        check(!HttpAsrPolicy.usable("openai", endpoint, "whisper-1", "")
                && !HttpAsrPolicy.usable("openai", endpoint, "whisper-1", "  "),
            "an empty token is refused");
        check(!HttpAsrPolicy.usable("openai", endpoint, "whisper\n1", "token")
                && !HttpAsrPolicy.usable("openai", endpoint + "\r", "whisper-1", "token"),
            "a control character is refused rather than smuggled into a header");
        check(!HttpAsrPolicy.usable("openai", endpoint + "\uD800", "whisper-1", "token"),
            "malformed Unicode in an endpoint is refused");
        check(!HttpAsrPolicy.usable("openai", endpoint, "whisper-1\uD800", "token")
                && !HttpAsrPolicy.usable("openai", endpoint, "whisper-1", "token\uD800"),
            "malformed Unicode in model and token is refused");
        check(!HttpAsrPolicy.usable("doubao", endpoint, "whisper-1", "token"),
            "an unsupported provider is not usable however complete it looks");
        check(!HttpAsrPolicy.usable("openai", null, "whisper-1", "token"),
            "a missing endpoint is refused rather than throwing");
        check(!HttpAsrPolicy.usable("openai", endpoint, "😀".repeat(200), "token")
                && !HttpAsrPolicy.usable("openai", endpoint, "whisper-1", "😀".repeat(5_000)),
            "model and token use UTF-8 byte bounds");

        // zh-CN and zh-TW are both zh to these APIs; which script comes back is this client's own
        // 简繁 setting, not the transcriber's to decide.
        check("zh".equals(HttpAsrPolicy.isoLanguage("zh-CN"))
                && "zh".equals(HttpAsrPolicy.isoLanguage("zh-TW"))
                && "zh".equals(HttpAsrPolicy.isoLanguage("zh_CN"))
                && "en".equals(HttpAsrPolicy.isoLanguage("en-US"))
                && "en".equals(HttpAsrPolicy.isoLanguage("en_US"))
                && "ja".equals(HttpAsrPolicy.isoLanguage("ja")),
            "a BCP 47 tag is reduced to its primary subtag");
        check("".equals(HttpAsrPolicy.isoLanguage(null))
                && "".equals(HttpAsrPolicy.isoLanguage("   ")),
            "an absent language stays absent");

        String boundary = HttpAsrPolicy.boundary("ime-abc-123");
        check(boundary.startsWith("msime") && boundary.contains("ime-abc-123"),
            "the boundary is derived from the request id");
        check(HttpAsrPolicy.boundary("a\r\nContent-Disposition: x").equals("msimeaContent-Dispositionx"),
            "anything that could break out of the body is dropped from the boundary");
        check(HttpAsrPolicy.boundary(null).equals("msime"), "a missing id still yields a boundary");

        byte[] wav = WavAudio.wrap(new byte[] {1, 2, 3, 4}, 4, WavAudio.SAMPLE_RATE);
        check(wav != null && wav.length == WavAudio.HEADER_BYTES + 4, "the WAV wraps the samples");
        check(new String(wav, 0, 4, StandardCharsets.US_ASCII).equals("RIFF")
                && new String(wav, 8, 4, StandardCharsets.US_ASCII).equals("WAVE")
                && new String(wav, 36, 4, StandardCharsets.US_ASCII).equals("data"),
            "the container has the chunks a decoder looks for");
        check(read32(wav, 4) == 40 && read32(wav, 40) == 4,
            "the declared sizes match the samples actually present");
        check(read32(wav, 24) == WavAudio.SAMPLE_RATE && read32(wav, 28) == WavAudio.SAMPLE_RATE * 2,
            "the sample and byte rates describe 16-bit mono");
        // An odd byte count cannot be whole samples; declaring a size the data does not have is
        // what makes a decoder read past the end.
        byte[] odd = WavAudio.wrap(new byte[] {1, 2, 3}, 3, WavAudio.SAMPLE_RATE);
        check(odd != null && odd.length == WavAudio.HEADER_BYTES + 2 && read32(odd, 40) == 2,
            "a trailing half sample is dropped rather than declared");
        check(WavAudio.wrap(new byte[] {1, 2}, 0, WavAudio.SAMPLE_RATE) == null
                && WavAudio.wrap(null, 4, WavAudio.SAMPLE_RATE) == null
                && WavAudio.wrap(new byte[] {1, 2}, 4, WavAudio.SAMPLE_RATE) == null
                && WavAudio.wrap(new byte[] {1, 2}, 2, 0) == null,
            "nothing to send yields nothing rather than a malformed file");

        byte[] body = HttpAsrPolicy.multipartBody(boundary, "whisper-1", "zh-CN", wav);
        String text = new String(body, StandardCharsets.ISO_8859_1);
        check(text.startsWith("--" + boundary + "\r\n"), "the body opens with the boundary");
        check(text.contains("name=\"model\"\r\n\r\nwhisper-1\r\n"), "the model is a field");
        check(text.contains("name=\"language\"\r\n\r\nzh\r\n"), "the language is sent reduced");
        check(text.contains("filename=\"audio.wav\"") && text.contains("Content-Type: audio/wav"),
            "the recording is the file part");
        check(text.endsWith("\r\n--" + boundary + "--\r\n"), "the body closes the multipart");
        check(body.length > wav.length, "the audio is carried whole inside the body");

        byte[] without = HttpAsrPolicy.multipartBody(boundary, "whisper-1", "  ", wav);
        check(!new String(without, StandardCharsets.ISO_8859_1).contains("name=\"language\""),
            "no language field is sent when there is none, which is how these APIs detect one");
        try {
            Method strictText = HttpAsrPolicy.class.getDeclaredMethod("strictText", Object.class);
            strictText.setAccessible(true);
            check("synthetic transcript".equals(strictText.invoke(null, "synthetic transcript")),
                "HTTP ASR accepts string transcripts");
            check("".equals(strictText.invoke(null, 42)),
                "HTTP ASR rejects numeric transcripts instead of coercing them");
            check("".equals(strictText.invoke(null, "字".repeat(2001))),
                "HTTP ASR rejects an oversized transcript before display");
            check("".equals(strictText.invoke(null, "好\u0000")),
                "HTTP ASR rejects control characters before display");
            check("".equals(strictText.invoke(null, "好\uD800")),
                "HTTP ASR rejects unpaired surrogates before display");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("HTTP ASR response parser unavailable", error);
        }
        System.out.println("Android HTTP ASR policy passed");
    }
}
