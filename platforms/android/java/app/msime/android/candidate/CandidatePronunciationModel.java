package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 把候选复制成有界的无会话离线查询，并校验音标回答。 */
public final class CandidatePronunciationModel {
    public static final int MAX_BYTES = 262_144;
    private CandidatePronunciationModel() {}

    public static String request(long generation, JSONArray candidates, boolean englishTarget,
            boolean glossVisible)
            throws JSONException {
        if (generation < 0 || candidates == null || candidates.length() > 4096)
            throw new IllegalArgumentException("Invalid pronunciation candidates");
        JSONArray items = new JSONArray();
        java.util.LinkedHashSet<String> texts = new java.util.LinkedHashSet<>();
        for (int index = 0; index < candidates.length(); index++) {
            JSONObject candidate = candidates.optJSONObject(index);
            if (candidate == null) continue;
            int source = InputViewValuePolicy.integer(candidate, "source", -1);
            String translation = glossVisible && CandidateTranslationPolicy.userTranslationSourceAllowed(source)
                ? InputViewValuePolicy.textOr(candidate, "translation", "") : "";
            String text = CandidatePronunciationPolicy.lookupText(
                InputViewValuePolicy.textOr(candidate, "text", ""), translation, englishTarget);
            if (!text.isEmpty() && TextPolicy.utf8Length(text) <= 4096) texts.add(text);
        }
        for (String text : texts) items.put(new JSONObject().put("text", text).put("language", "en"));
        String result = new JSONObject().put("generation", generation).put("items", items).toString();
        if (TextPolicy.utf8Length(result) > MAX_BYTES)
            throw new IllegalArgumentException("Pronunciation request is too large");
        return result;
    }

    public static Map<String, String> decode(String response, long generation) throws JSONException {
        if (response == null || TextPolicy.utf8Length(response) > 1_048_576)
            throw new IllegalArgumentException("Pronunciation response is too large");
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok"))) throw new JSONException("Pronunciation lookup failed");
        JSONObject value = envelope.getJSONObject("value");
        if (CandidateGlossPolicy.strictOr(value.opt("generation"), -1) != generation)
            throw new JSONException("Stale pronunciation generation");
        JSONArray entries = value.getJSONArray("pronunciations");
        if (entries.length() > 4096) throw new JSONException("Too many pronunciations");
        Map<String, String> readings = new LinkedHashMap<>();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.getJSONObject(index);
            if (!"en".equals(entry.getString("language"))) continue;
            String text = entry.getString("text");
            String reading = entry.getString("pronunciation");
            if (text.isEmpty() || TextPolicy.utf8Length(text) > 4096 || reading.isEmpty()
                    || TextPolicy.utf8Length(reading) > 4096 || !reading.startsWith("/")
                    || !reading.endsWith("/")) continue;
            readings.put(text, reading);
        }
        return Map.copyOf(readings);
    }
}
