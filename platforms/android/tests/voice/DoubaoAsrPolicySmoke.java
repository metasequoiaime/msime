import app.msime.android.DoubaoAsrPolicy;
import app.msime.android.HttpAsrPolicy;
import java.util.Arrays;
import java.util.List;
import java.lang.reflect.Method;

/** Which requests are the streaming protocol, and what a half-configured account must not open. */
public final class DoubaoAsrPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static List<String> headers(String... names) {
        return Arrays.asList(names);
    }

    public static void main(String[] args) {
        String endpoint = "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async";
        // The names the shared auth policy produces: resource and request always, then either the
        // single API key or the app-key/access-key pair depending on the mode.
        List<String> apiKey = headers("x-api-resource-id", "x-api-request-id", "x-api-key");
        List<String> legacy = headers("x-api-resource-id", "x-api-request-id",
            "x-api-app-key", "x-api-access-key");

        check(DoubaoAsrPolicy.isStreaming("doubao"), "doubao is the streaming provider");
        check(!DoubaoAsrPolicy.isStreaming("openai") && !DoubaoAsrPolicy.isStreaming(null),
            "nothing else is");
        // The two protocols are disjoint: a request qualifies for exactly one path.
        check(!HttpAsrPolicy.supported("doubao"), "doubao is not an upload provider");
        check(!DoubaoAsrPolicy.usable("openai", endpoint, apiKey),
            "an upload provider is not opened as a stream");

        check(DoubaoAsrPolicy.usable("doubao", endpoint, apiKey), "the api-key mode is usable");
        check(DoubaoAsrPolicy.usable("doubao", endpoint, legacy), "the legacy mode is usable");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-request-id", "x-api-key", "x-unexpected")),
            "unknown headers are refused rather than forwarded to the handshake");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-request-id", "x-api-key", "x-api-key")),
            "duplicate credentials are refused rather than sent twice");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-request-id", "x-api-app-key",
                    "x-api-access-key", "x-api-key")),
            "mixed credential modes are refused rather than sent together");

        // wss only: the credentials travel in the handshake's own headers, and this host opens the
        // connection, so a downgrade would put them on the wire in clear text.
        check(!DoubaoAsrPolicy.usable("doubao", "ws://openspeech.bytedance.com/x", apiKey),
            "a plaintext endpoint is refused");
        check(!DoubaoAsrPolicy.usable("doubao", "https://openspeech.bytedance.com/x", apiKey),
            "an https endpoint is not this protocol");
        check(!DoubaoAsrPolicy.usable("doubao", "wss:///asr", apiKey),
            "an endpoint without an authority is refused");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint + "\n", apiKey),
            "a control character is refused rather than smuggled into the handshake");
        check(!DoubaoAsrPolicy.usable("doubao", null, apiKey),
            "a missing endpoint is refused rather than throwing");

        // A half-configured account fails here rather than at a socket with the user waiting.
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-request-id", "x-api-key")),
            "without the resource id the session cannot start");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-key")),
            "without a request id the session cannot start");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-request-id", "x-api-app-key")),
            "app-key without access-key is not a credential");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint, headers()),
            "no headers at all is not configured");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint, null),
            "absent headers are refused rather than throwing");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
                headers("x-api-resource-id", "x-api-request-id", null)),
            "a null header name is refused");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
            headers("x-api-resource-id", "x-api-request-id", "x-api-key\r")),
            "a header name carrying a control character is refused");
        check(!DoubaoAsrPolicy.usable("doubao", endpoint,
            headers("x-api-resource-id", "x-api-request-id", "x-api-key\uD800")),
            "a header name carrying malformed Unicode is refused");
        try {
            Method strictText = DoubaoAsrPolicy.class.getDeclaredMethod("strictText", Object.class);
            strictText.setAccessible(true);
            check("synthetic transcript".equals(strictText.invoke(null, "synthetic transcript")),
                "Doubao accepts string transcripts");
            check("".equals(strictText.invoke(null, 42)),
                "Doubao rejects numeric transcripts instead of coercing them");
            check("".equals(strictText.invoke(null, "好\u0000")),
                "Doubao rejects control characters before display");
            check("".equals(strictText.invoke(null, "好\uD800")),
                "Doubao rejects unpaired surrogates before display");
            Method strictPayload = DoubaoAsrPolicy.class.getDeclaredMethod("strictPayload", Object.class);
            strictPayload.setAccessible(true);
            check("{\"result\":{}}".equals(strictPayload.invoke(null, "{\"result\":{}}")),
                "Doubao accepts string payloads");
            check(strictPayload.invoke(null, 42) == null,
                "Doubao rejects numeric payloads instead of coercing them");
            check(strictPayload.invoke(null, new Object()) == null,
                "Doubao rejects object payloads instead of coercing them");
            Method strictBoolean = DoubaoAsrPolicy.class.getDeclaredMethod("strictBoolean", Object.class);
            strictBoolean.setAccessible(true);
            check(Boolean.TRUE.equals(strictBoolean.invoke(null, Boolean.TRUE)),
                "Doubao accepts JSON booleans");
            check(strictBoolean.invoke(null, "true") == null,
                "Doubao rejects boolean strings instead of coercing them");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("Doubao response parser unavailable", error);
        }
        System.out.println("Android Doubao streaming policy passed");
    }
}
