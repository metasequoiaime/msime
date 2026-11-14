package app.msime.android;

import org.json.JSONException;
import org.json.JSONObject;

/** 阿里云目录、配置校验与选路均向共享层查询，桥接不可用时不联网、不回落。 */
public final class AliyunTranslationPolicy {
    private AliyunTranslationPolicy() {}

    public static JSONObject call(JSONObject request) {
        try {
            JSONObject envelope = new JSONObject(NativeClient.aliyunTranslation(request.toString()));
            return JsonPolicy.strictTrue(envelope.opt("ok")) ? envelope.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    public static JSONObject catalog() {
        try { return call(new JSONObject().put("operation", "catalog")); }
        catch (JSONException impossible) { return null; }
    }

    public static boolean enabled(JSONObject config) {
        return config != null && JsonPolicy.strictTrue(config.opt("enabled"));
    }

    public static CandidateTranslationRoute route(boolean translations, JSONObject config) {
        if (!translations || !enabled(config)) return CandidateTranslationRoute.NONE;
        try {
            JSONObject result = call(new JSONObject().put("operation", "validate").put("config", config));
            if (result == null || !JsonPolicy.strictTrue(result.opt("ready"))) return CandidateTranslationRoute.NONE;
            return CandidateTranslationRoute.aliyun(text(config, "access_key_id"), text(config, "access_key_secret"),
                text(config, "region"), text(config, "edition"), text(config, "scene"));
        } catch (JSONException impossible) { return CandidateTranslationRoute.NONE; }
    }

    public static String issue(JSONObject config, String key, String value) {
        try {
            JSONObject updated = new JSONObject(config.toString()).put(key, value);
            JSONObject result = call(new JSONObject().put("operation", "validate").put("config", updated));
            if (result == null) return "共享配置校验暂不可用，请稍后重试";
            return result.isNull("issue") ? null : JsonPolicy.strictStringOrEmpty(result.opt("issue"));
        } catch (JSONException impossible) { return "读取配置失败"; }
    }

    public static String text(JSONObject config, String key) {
        if (config != null && config.has(key)) return JsonPolicy.strictStringOrEmpty(config.opt(key));
        JSONObject catalog = catalog();
        JSONObject defaults = catalog == null ? null : catalog.optJSONObject("defaults");
        return defaults == null ? "" : JsonPolicy.strictStringOrEmpty(defaults.opt(key));
    }

    public static JSONObject config(CandidateTranslationRoute route) throws JSONException {
        return new JSONObject().put("enabled", true).put("access_key_id", route.secretId())
            .put("access_key_secret", route.secretKey()).put("region", route.region())
            .put("edition", route.aliyunEdition()).put("scene", route.aliyunScene());
    }

    public static JSONObject parse(byte[] body) throws JSONException {
        return call(new JSONObject().put("operation", "parse").put("body_base64",
            java.util.Base64.getEncoder().encodeToString(body)));
    }

    public static JSONObject request(CandidateTranslationRoute route, String word, String language) throws JSONException {
        return call(new JSONObject().put("operation", "request").put("config", config(route))
            .put("text", word).put("source_language", "zh").put("target_language", language));
    }
}
