package app.msime.android;

import java.util.Iterator;
import org.json.JSONException;
import org.json.JSONObject;

/** 语音服务切换的 JSON 适配层；换槽和字段策略由共享 Rust 计算，不记录凭据。 */
public final class VoiceProviderSettings {
    private VoiceProviderSettings() {}

    /** 在待保存的语音文档上应用补丁；调用方在工作线程读最新偏好，失败时不能保存。 */
    public static void switchProvider(JSONObject voice, String prefix, String provider) throws JSONException {
        if (voice == null || !("asr".equals(prefix) || "polish".equals(prefix))
                || provider == null || provider.isEmpty()) throw invalid();
        if (provider.equals(JsonPolicy.strictStringOrEmpty(voice.opt(prefix + "_provider")))) return;
        JSONObject current = new JSONObject();
        for (String field : new String[] {"provider", "endpoint", "model", "token"})
            current.put(field, JsonPolicy.strictStringOrEmpty(voice.opt(prefix + "_" + field)));
        current.put("tokens", stringSlots(voice.optJSONObject(prefix + "_tokens")));
        JSONObject request = new JSONObject().put("service", "voice." + prefix).put("provider", provider)
            .put("field_policy", "clear").put("current", current);
        try {
            applyPatch(voice, prefix, provider, NativeClient.voiceProviderUpdate(request.toString()));
        } catch (IllegalArgumentException | LinkageError error) {
            throw invalid();
        }
    }

    /** 旧设置只消费字符串槽位；不把未定型 JSON 的数字和布尔值强转成凭据。 */
    private static JSONObject stringSlots(JSONObject source) throws JSONException {
        JSONObject slots = new JSONObject();
        if (source != null) {
            for (Iterator<String> keys = source.keys(); keys.hasNext(); ) {
                String key = keys.next();
                Object value = source.opt(key);
                if (value instanceof String) slots.put(key, value);
            }
        }
        return slots;
    }

    /** 完整核对后再修改文档，避免错误响应写下一半字段或沿用旧服务的自定义地址。 */
    private static void applyPatch(JSONObject voice, String prefix, String provider, String response)
            throws JSONException {
        JSONObject patch;
        JSONObject slots = new JSONObject();
        try {
            patch = response == null ? null : JsonPolicy.objectValue(new JSONObject(response));
            if (patch == null || !provider.equals(JsonPolicy.strictString(patch.opt("provider")))
                    || JsonPolicy.strictString(patch.opt("token")) == null
                    || !"".equals(JsonPolicy.strictString(patch.opt("endpoint")))
                    || !"".equals(JsonPolicy.strictString(patch.opt("model")))) throw invalid();
            JSONObject tokens = patch.getJSONObject("tokens");
            for (Iterator<String> keys = tokens.keys(); keys.hasNext(); ) {
                String key = keys.next();
                String token = JsonPolicy.strictString(tokens.opt(key));
                if (token == null) throw invalid();
                slots.put(key, token);
            }
        } catch (JSONException error) {
            throw invalid();
        }
        for (String field : new String[] {"provider", "token", "endpoint", "model"})
            voice.put(prefix + "_" + field, patch.getString(field));
        voice.put(prefix + "_tokens", slots);
    }

    private static JSONException invalid() { return new JSONException("voice_provider_update_failed"); }
}
