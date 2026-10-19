package app.msime.android;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 共享 Rust 语音服务目录的持有快照；不保存偏好或凭据，Java 只追加平台服务。 */
public final class VoiceProviderCatalog {
    public record Provider(String id, String title) {}
    public record StreamEndpoint(String id, String label, String title, String endpoint) {}

    private final List<Provider> asr;
    private final List<Provider> polish;
    private final List<StreamEndpoint> doubaoEndpoints;
    private final String defaultDoubaoEndpoint;

    VoiceProviderCatalog(List<Provider> cloudAsr, List<Provider> polish,
            List<StreamEndpoint> doubaoEndpoints, String defaultDoubaoEndpoint) {
        var choices = new ArrayList<Provider>();
        if (!cloudAsr.isEmpty()) {
            choices.add(new Provider(VoiceProviderPolicy.SYSTEM, "系统识别"));
            choices.add(new Provider(VoiceProviderPolicy.LOCAL, "本地模型"));
            choices.addAll(cloudAsr);
        }
        this.asr = List.copyOf(choices);
        this.polish = List.copyOf(polish);
        this.doubaoEndpoints = List.copyOf(doubaoEndpoints);
        this.defaultDoubaoEndpoint = defaultDoubaoEndpoint;
    }

    /** 纯计算 JNI；失败快照不提供猜测的服务或接口，不改当前配置。 */
    public static VoiceProviderCatalog read() {
        try {
            return parse(NativeClient.voiceProviderCatalog());
        } catch (JSONException | IllegalArgumentException | LinkageError error) {
            return unavailable();
        }
    }

    public static VoiceProviderCatalog unavailable() {
        return new VoiceProviderCatalog(List.of(), List.of(), List.of(), "");
    }

    /** 只接受完整成功信封与字符串字段，避免损坏目录变成不完整的可选列表。 */
    public static VoiceProviderCatalog parse(String json) throws JSONException {
        if (json == null) throw new JSONException("voice_catalog_invalid");
        JSONObject value = JsonPolicy.objectValue(new JSONObject(json));
        if (value == null) throw new JSONException("voice_catalog_invalid");
        JSONArray rawAsr = value.getJSONArray("asr");
        List<Provider> asr = providers(rawAsr);
        List<Provider> polish = providers(value.getJSONArray("polish"));
        String defaultEndpoint = "";
        for (int index = 0; index < rawAsr.length(); index++) {
            JSONObject entry = rawAsr.getJSONObject(index);
            if (DoubaoAsrPolicy.PROVIDER.equals(entry.opt("id"))) defaultEndpoint = text(entry, "endpoint");
        }
        if (asr.stream().anyMatch(entry -> VoiceProviderPolicy.SYSTEM.equals(entry.id())
                || VoiceProviderPolicy.LOCAL.equals(entry.id())))
            throw new JSONException("voice_catalog_invalid");
        var endpoints = new ArrayList<StreamEndpoint>();
        Set<String> ids = new HashSet<>();
        Set<String> addresses = new HashSet<>();
        JSONArray rawEndpoints = value.getJSONArray("doubao_stream_endpoints");
        for (int index = 0; index < rawEndpoints.length(); index++) {
            JSONObject entry = rawEndpoints.getJSONObject(index);
            var endpoint = new StreamEndpoint(text(entry, "id"), text(entry, "label"),
                text(entry, "title"), text(entry, "endpoint"));
            if (!ids.add(endpoint.id()) || !addresses.add(endpoint.endpoint()))
                throw new JSONException("voice_catalog_invalid");
            endpoints.add(endpoint);
        }
        if (endpoints.isEmpty() || !addresses.contains(defaultEndpoint))
            throw new JSONException("voice_catalog_invalid");
        return new VoiceProviderCatalog(asr, polish, endpoints, defaultEndpoint);
    }

    private static List<Provider> providers(JSONArray entries) throws JSONException {
        var result = new ArrayList<Provider>();
        Set<String> ids = new HashSet<>();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.getJSONObject(index);
            var provider = new Provider(text(entry, "id"), text(entry, "title"));
            if (!ids.add(provider.id())) throw new JSONException("voice_catalog_invalid");
            result.add(provider);
        }
        if (result.isEmpty()) throw new JSONException("voice_catalog_invalid");
        return result;
    }

    private static String text(JSONObject value, String key) throws JSONException {
        String result = JsonPolicy.strictString(value.opt(key));
        if (result == null || result.trim().isEmpty()) throw new JSONException("voice_catalog_invalid");
        return result;
    }

    public boolean available() { return !asr.isEmpty() && !polish.isEmpty() && !doubaoEndpoints.isEmpty(); }
    public List<Provider> asr() { return asr; }
    public List<Provider> polish() { return polish; }
    public List<StreamEndpoint> doubaoEndpoints() { return doubaoEndpoints; }

    public String asrLabel(String id) {
        if (id == null || id.isEmpty() || VoiceProviderPolicy.SYSTEM.equals(id)) return "系统识别";
        if (VoiceProviderPolicy.LOCAL.equals(id)) return "本地模型";
        return label(asr, id);
    }

    public String polishLabel(String id) {
        if (id == null || id.isEmpty()) return polish.isEmpty() ? "未设置" : polish.get(0).title();
        return label(polish, id);
    }

    private static String label(List<Provider> providers, String id) {
        for (Provider entry : providers) if (entry.id().equals(id)) return entry.title();
        return id;
    }

    public StreamEndpoint selectedEndpoint(String endpoint) {
        String value = endpoint == null ? "" : endpoint.trim();
        if (value.isEmpty()) value = defaultDoubaoEndpoint;
        for (StreamEndpoint entry : doubaoEndpoints) if (entry.endpoint().equals(value)) return entry;
        return null;
    }

    public String doubaoEndpointLabel(String endpoint) {
        StreamEndpoint selected = selectedEndpoint(endpoint);
        return selected == null ? "自定义地址" : selected.label();
    }
}
