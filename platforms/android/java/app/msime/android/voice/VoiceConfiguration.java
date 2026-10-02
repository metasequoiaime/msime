package app.msime.android;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * What the keyboard's own voice entry should use, decoded from the shared resolution.
 *
 * <p>The settings app and this keyboard now ask the same question of the same document through
 * `msime_client_mobile_voice_configuration`. Nothing is resolved here — no provider defaults, no
 * endpoint or credential rules — because a second copy of those in Java is how the two entries
 * would start disagreeing about which service a device transcribes with.
 *
 * <p>Everything absent means "not configured", which is not an error: this host falls back to the
 * platform recogniser, and that remains what a user who set nothing up gets.
 */
public final class VoiceConfiguration {
    private final String providerName;
    private final String endpoint;
    private final String model;
    private final String token;
    private final VoiceRecognitionActivity.Streaming streaming;
    private final VoiceRecognitionActivity.Polish polish;
    private final String localModel;

    private VoiceConfiguration(String providerName, String endpoint, String model, String token,
                               VoiceRecognitionActivity.Streaming streaming,
                               VoiceRecognitionActivity.Polish polish) {
        this(providerName, endpoint, model, token, streaming, polish, null);
    }

    private VoiceConfiguration(String providerName, String endpoint, String model, String token,
                               VoiceRecognitionActivity.Streaming streaming,
                               VoiceRecognitionActivity.Polish polish, String localModel) {
        this.providerName = providerName;
        this.endpoint = endpoint;
        this.model = model;
        this.token = token;
        this.streaming = streaming;
        this.polish = polish;
        this.localModel = localModel;
    }

    /** Nothing configured: the platform recogniser, with no rewrite. */
    public static VoiceConfiguration none() {
        return new VoiceConfiguration(null, null, null, null, null, null);
    }

    /** The provider this request will use, or null for the platform recogniser. */
    public String provider() {
        return streaming != null ? DoubaoAsrPolicy.PROVIDER : providerName;
    }

    public String providerName() {
        return providerName;
    }

    public String endpoint() {
        return endpoint;
    }

    public String model() {
        return model;
    }

    public String token() {
        return token;
    }

    public VoiceRecognitionActivity.Streaming streaming() {
        return streaming;
    }

    public VoiceRecognitionActivity.Polish polish() {
        return polish;
    }

    /** The installed model directory for on-device recognition, or null for every other engine. */
    public String localModel() {
        return localModel;
    }

    /** Read the shared resolution for this preferences directory; failures mean "not configured". */
    public static VoiceConfiguration read(String directory, String requestId) {
        if (directory == null || directory.isEmpty()) return none();
        try {
            return decode(NativeClient.mobileVoiceConfiguration(directory), requestId);
        } catch (RuntimeException | LinkageError error) {
            return none();
        }
    }

    /**
     * Decode one shared answer.
     *
     * <p>A request qualifies for exactly one transport: on-device recognition, Doubao's streaming
     * socket or the OpenAI-compatible upload. Anything the host cannot speak leaves all three null
     * and the platform recogniser runs, rather than the voice button failing.
     */
    public static VoiceConfiguration decode(String response, String requestId) {
        try {
            JSONObject document = new JSONObject(response);
            if (!document.optBoolean("ok", false)) return none();
            JSONObject value = document.optJSONObject("value");
            if (value == null) return none();
            JSONObject provider = value.optJSONObject("provider");
            JSONObject polishValue = value.optJSONObject("polish");
            VoiceRecognitionActivity.Polish polish = polish(polishValue, requestId);
            if (provider == null) return new VoiceConfiguration(null, null, null, null, null, polish);
            String name = provider.optString("provider", "");
            String endpoint = provider.optString("endpoint", "");
            String model = provider.optString("model", "");
            String token = provider.optString("token", "");
            String[] headers = headers(provider.optJSONArray("headers"));
            String modelPath = provider.isNull("modelPath") ? "" : provider.optString("modelPath", "");
            if (LocalAsrPolicy.usable(name, modelPath)) {
                return new VoiceConfiguration(name, null, null, null, null, polish, modelPath);
            }
            if (DoubaoAsrPolicy.usable(name, endpoint, java.util.Arrays.asList(names(headers)))) {
                return new VoiceConfiguration(name, null, null, null,
                    new VoiceRecognitionActivity.Streaming(endpoint, headers,
                        provider.optBoolean("enableItn", false),
                        provider.optBoolean("enablePunctuation", false),
                        provider.optBoolean("enableDdc", false),
                        provider.optString("boostingTableId", "")),
                    polish);
            }
            if (HttpAsrPolicy.usable(name, endpoint, model, token)) {
                return new VoiceConfiguration(name, endpoint, model, token, null, polish);
            }
            return new VoiceConfiguration(null, null, null, null, null, polish);
        } catch (JSONException error) {
            return none();
        }
    }

    private static VoiceRecognitionActivity.Polish polish(JSONObject value, String requestId) {
        if (value == null) return null;
        String prompt = NativeClient.polishPrompt(value.optString("promptId", ""),
            value.optString("promptCustom1", ""), value.optString("promptCustom2", ""),
            value.optString("promptCustom3", ""));
        String endpoint = value.optString("endpoint", "");
        String model = value.optString("model", "");
        String token = value.optString("token", "");
        if (!VoicePolishPolicy.usable(endpoint, model, token, prompt)) return null;
        return new VoiceRecognitionActivity.Polish(endpoint, model, token, prompt);
    }

    /** Flattened name/value pairs, the shape the handshake takes them in. */
    static String[] headers(JSONArray value) {
        if (value == null) return new String[0];
        String[] flat = new String[value.length() * 2];
        for (int index = 0; index < value.length(); index++) {
            JSONObject header = value.optJSONObject(index);
            if (header == null) return new String[0];
            flat[index * 2] = header.optString("name", "");
            flat[index * 2 + 1] = header.optString("value", "");
        }
        return flat;
    }

    static String[] names(String[] flat) {
        String[] out = new String[flat.length / 2];
        for (int index = 0; index < out.length; index++) out[index] = flat[index * 2];
        return out;
    }
}
