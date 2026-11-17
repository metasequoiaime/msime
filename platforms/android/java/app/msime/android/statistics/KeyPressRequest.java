package app.msime.android;

import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/** 批次到共享统计契约的适配；总数同时保留，兼容忽略 sources 的旧共享库。 */
public final class KeyPressRequest {
    private KeyPressRequest() {}

    public static String encode(String directory, KeyPressBatch.Flush batch) throws JSONException {
        JSONObject sources = new JSONObject();
        for (Map.Entry<TypingSource, Map<String, Long>> source : batch.sources().entrySet())
            sources.put(source.getKey().id(), counts(source.getValue()));
        return new JSONObject().put("directory", directory).put("action", new JSONObject()
            .put("operation", "record_keys").put("day", batch.day())
            .put("keys", counts(batch.keys())).put("sources", sources)).toString();
    }

    private static JSONObject counts(Map<String, Long> counts) throws JSONException {
        JSONObject result = new JSONObject();
        for (Map.Entry<String, Long> entry : counts.entrySet())
            result.put(entry.getKey(), entry.getValue().longValue());
        return result;
    }
}
