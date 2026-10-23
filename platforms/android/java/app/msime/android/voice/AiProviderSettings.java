package app.msime.android;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** AI 目录和切换补丁的 JSON 适配；公开预设与规则来自共享 Rust，不记录凭据。 */
public final class AiProviderSettings {
    public record Provider(String id, String title, List<String> models, String documentation, boolean desktopOnly, boolean usesToken) {
        public Provider { models = List.copyOf(models); }
    }

    private AiProviderSettings() {}

    public static List<Provider> catalog() {
        try {
            return parseCatalog(NativeClient.aiProviderCatalog());
        } catch (JSONException | IllegalArgumentException | LinkageError error) {
            return List.of();
        }
    }

    public static List<Provider> parseCatalog(String response) throws JSONException {
        JSONArray entries = arrayValue(response);
        var result = new ArrayList<Provider>();
        Set<String> ids = new HashSet<>();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.getJSONObject(index);
            String id = text(entry, "id");
            String title = text(entry, "title");
            String documentation = text(entry, "documentation");
            if (id.isEmpty() || title.isEmpty() || !ids.add(id) || !(entry.opt("desktop_only") instanceof Boolean)
                    || !(entry.opt("uses_token") instanceof Boolean))
                throw invalid();
            result.add(new Provider(id, title, strings(entry.getJSONArray("models")), documentation,
                entry.getBoolean("desktop_only"), entry.getBoolean("uses_token")));
        }
        if (result.isEmpty()) throw invalid();
        return List.copyOf(result);
    }

    /** 只传公开字段，并在完整核对补丁后更新文档；凭据槽和提示词保持原值。接口地址/模型的每提供方槽位随补丁写回文档。 */
    public static void switchProvider(JSONObject ai, String provider) throws JSONException {
        JSONObject current = new JSONObject().put("enabled", ai.optBoolean("enabled", false));
        for (String key : new String[] {"provider", "endpoint", "model"})
            current.put(key, JsonPolicy.strictStringOrEmpty(ai.opt(key)));
        for (String key : new String[] {"endpoints", "models"}) {
            Object slots = ai.opt(key);
            current.put(key, slots instanceof JSONObject ? slots : new JSONObject());
        }
        JSONObject request = new JSONObject().put("provider", provider).put("current", current);
        JSONObject patch;
        try {
            patch = JsonPolicy.objectValue(new JSONObject(NativeClient.aiProviderUpdate(request.toString())));
            if (patch == null || !provider.equals(text(patch, "provider"))
                    || !(patch.opt("enabled") instanceof Boolean)) throw invalid();
            text(patch, "endpoint");
            text(patch, "model");
            if (!(patch.opt("endpoints") instanceof JSONObject) || !(patch.opt("models") instanceof JSONObject))
                throw invalid();
        } catch (IllegalArgumentException | LinkageError error) {
            throw invalid();
        }
        for (String key : new String[] {"enabled", "provider", "endpoint", "model", "endpoints", "models"})
            ai.put(key, patch.get(key));
    }

    public static List<String> fetchModels(String endpoint, String token) throws JSONException {
        JSONObject request = new JSONObject().put("endpoint", endpoint).put("token", token);
        return strings(arrayValue(NativeClient.aiModels(request.toString())));
    }

    private static JSONArray arrayValue(String response) throws JSONException {
        if (response == null) throw invalid();
        JSONObject envelope = new JSONObject(response);
        if (!Boolean.TRUE.equals(envelope.opt("ok")) || !(envelope.opt("value") instanceof JSONArray)) throw invalid();
        return envelope.getJSONArray("value");
    }

    private static List<String> strings(JSONArray values) throws JSONException {
        var result = new java.util.LinkedHashSet<String>();
        for (int index = 0; index < values.length(); index++) {
            String value = JsonPolicy.strictString(values.get(index));
            if (value == null || value.isEmpty()) throw invalid();
            result.add(value);
        }
        return List.copyOf(result);
    }

    private static String text(JSONObject object, String key) throws JSONException {
        String value = JsonPolicy.strictString(object.opt(key));
        if (value == null) throw invalid();
        return value;
    }

    private static JSONException invalid() { return new JSONException("ai_settings_invalid"); }
}
