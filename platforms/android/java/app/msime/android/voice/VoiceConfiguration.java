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
     * socket or the OpenAI-compatible upload. 网络 provider 无法使用时才回到系统识别；明确选择
     * local 却没有可用模型时必须保留这个选择，让上层报告本地模型错误。
     */
    public static VoiceConfiguration decode(String response, String requestId) {
        try {
            JSONObject document = new JSONObject(response);
            if (!JsonPolicy.strictTrue(document.opt("ok"))) return none();
            JSONObject value = document.optJSONObject("value");
            if (value == null) return none();
            JSONObject provider = value.optJSONObject("provider");
            JSONObject polishValue = value.optJSONObject("polish");
            VoiceRecognitionActivity.Polish polish = polish(polishValue, requestId);
            if (provider == null) return new VoiceConfiguration(null, null, null, null, null, polish);
            String name = JsonPolicy.strictStringOrEmpty(provider.opt("provider"));
            String endpoint = JsonPolicy.strictStringOrEmpty(provider.opt("endpoint"));
            String model = JsonPolicy.strictStringOrEmpty(provider.opt("model"));
            String token = JsonPolicy.strictStringOrEmpty(provider.opt("token"));
            String[] headers = headers(provider.optJSONArray("headers"));
            String modelPath = JsonPolicy.strictStringOrEmpty(provider.opt("modelPath"));
            if ("local".equals(name)) {
                return fromProvider(name, modelPath, polish);
            }
            if (LocalAsrPolicy.usable(name, modelPath)) {
                return new VoiceConfiguration(name, null, null, null, null, polish, modelPath);
            }
            if (DoubaoAsrPolicy.usable(name, endpoint, java.util.Arrays.asList(names(headers)))) {
                return new VoiceConfiguration(name, null, null, null,
                    new VoiceRecognitionActivity.Streaming(endpoint, headers,
                        JsonPolicy.strictTrue(provider.opt("enableItn")),
                        JsonPolicy.strictTrue(provider.opt("enablePunctuation")),
                        JsonPolicy.strictTrue(provider.opt("enableDdc")),
                        JsonPolicy.strictStringOrEmpty(provider.opt("boostingTableId"))),
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

    /** 即使本机无法使用路径，也保留明确选择的本地识别，不能静默切到系统云端识别。 */
    static VoiceConfiguration fromProvider(String name, String modelPath,
                                           VoiceRecognitionActivity.Polish polish) {
        if (!"local".equals(name)) return none();
        return new VoiceConfiguration(name, null, null, null, null, polish, modelPath);
    }

    private static VoiceRecognitionActivity.Polish polish(JSONObject value, String requestId) {
        if (value == null) return null;
        String prompt = NativeClient.polishPrompt(
            JsonPolicy.strictStringOrEmpty(value.opt("promptId")),
            JsonPolicy.strictStringOrEmpty(value.opt("promptCustom1")),
            JsonPolicy.strictStringOrEmpty(value.opt("promptCustom2")),
            JsonPolicy.strictStringOrEmpty(value.opt("promptCustom3")));
        String endpoint = JsonPolicy.strictStringOrEmpty(value.opt("endpoint"));
        String model = JsonPolicy.strictStringOrEmpty(value.opt("model"));
        String token = JsonPolicy.strictStringOrEmpty(value.opt("token"));
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
            flat[index * 2] = JsonPolicy.strictStringOrEmpty(header.opt("name"));
            flat[index * 2 + 1] = JsonPolicy.strictStringOrEmpty(header.opt("value"));
        }
        return flat;
    }

    static String[] names(String[] flat) {
        String[] out = new String[flat.length / 2];
        for (int index = 0; index < out.length; index++) out[index] = flat[index * 2];
        return out;
    }
}
