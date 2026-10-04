import app.msime.android.VoicePolisher;
import java.lang.reflect.Method;
import java.util.Map;

/** Cancellation is checked before opening a network connection. */
public final class VoicePolisherSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        VoicePolisher cancelled = new VoicePolisher();
        cancelled.cancel();
        check(cancelled.polish("https://127.0.0.1:1/v1/chat/completions", "m", "token", "p", "你好") == null,
            "a cancelled polish does not start a request");
        Map<String, String> anthropic = VoicePolisher.authenticationHeaders(
            "https://api.anthropic.com/v1/chat/completions", "fixture-token");
        check("fixture-token".equals(anthropic.get("x-api-key"))
                && "2023-06-01".equals(anthropic.get("anthropic-version"))
                && !anthropic.containsKey("Authorization"),
            "Anthropic polishing uses its API-key headers");
        Map<String, String> compatible = VoicePolisher.authenticationHeaders(
            "https://api.example.test/v1/chat/completions", "fixture-token");
        check("Bearer fixture-token".equals(compatible.get("Authorization"))
                && !compatible.containsKey("x-api-key"),
            "OpenAI-compatible polishing keeps bearer authentication");
        Method content;
        try {
            content = VoicePolisher.class.getDeclaredMethod("strictContent", Object.class);
            content.setAccessible(true);
            check("polished".equals(content.invoke(null, "polished")),
                "voice polish accepts string content");
            check("".equals(content.invoke(null, 42)),
                "voice polish rejects numeric content instead of coercing it");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("voice polish response parser unavailable", error);
        }
        System.out.println("Android voice polisher cancellation passed");
    }
}
