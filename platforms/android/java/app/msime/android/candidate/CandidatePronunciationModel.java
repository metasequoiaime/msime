package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 把候选复制成有界的无会话离线查询，并校验读音回答；请求和回答都按 {@link CandidatePronunciationPolicy#key} 对应。 */
public final class CandidatePronunciationModel {
    public static final int MAX_BYTES = 262_144;
    private CandidatePronunciationModel() {}

    public static String request(long generation, JSONArray candidates, String target,
            boolean glossVisible)
            throws JSONException {
        return request(generation, candidates, target, glossVisible, java.util.List.of());
    }

    /** 引擎候选和英文直输建议共用一批读音查询，重复的键只查一次。{@code target} 是释义的目标语言。 */
    public static String request(long generation, JSONArray candidates, String target,
            boolean glossVisible, java.util.List<String> englishSuggestions) throws JSONException {
        if (generation < 0 || candidates == null || candidates.length() > 4096)
            throw new IllegalArgumentException("Invalid pronunciation candidates");
        JSONArray items = new JSONArray();
        java.util.LinkedHashSet<String> keys = new java.util.LinkedHashSet<>();
        for (int index = 0; index < candidates.length(); index++) {
            JSONObject candidate = candidates.optJSONObject(index);
            if (candidate == null) continue;
            int source = InputViewValuePolicy.integer(candidate, "source", -1);
            String translation = glossVisible && CandidateTranslationPolicy.userTranslationSourceAllowed(source)
                ? InputViewValuePolicy.textOr(candidate, "translation", "") : "";
            String key = CandidatePronunciationPolicy.lookupText(
                InputViewValuePolicy.textOr(candidate, "text", ""), translation, target);
            if (!key.isEmpty() && TextPolicy.utf8Length(key) <= 4096) keys.add(key);
        }
        for (String suggestion : englishSuggestions) {
            String key = CandidatePronunciationPolicy.lookupText(suggestion, "", "en");
            if (!key.isEmpty() && TextPolicy.utf8Length(key) <= 4096) keys.add(key);
        }
        for (String key : keys) {
            int cut = key.indexOf('\n');
            items.put(new JSONObject().put("text", key.substring(cut + 1)).put("language", key.substring(0, cut)));
        }
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
            String language = entry.getString("language");
            if (!CandidatePronunciationPolicy.LANGUAGES.contains(language)) continue;
            String text = entry.getString("text");
            String reading = entry.getString("pronunciation");
            if (text.isEmpty() || TextPolicy.utf8Length(text) > 4096 || reading.isEmpty()
                    || TextPolicy.utf8Length(reading) > 4096 || !wellFormed(language, reading)) continue;
            readings.put(CandidatePronunciationPolicy.key(language, text), reading);
        }
        return Map.copyOf(readings);
    }

    /** 一条读音的形状对不对：英文是 `/…/`，日文和韩文是罗马字，其余是 `/…/` 或 `[…]`。 */
    public static boolean wellFormed(String language, String reading) {
        if ("ja".equals(language) || "ko".equals(language)) return !reading.trim().isEmpty();
        if (reading.length() < 3) return false;
        if (reading.startsWith("/") && reading.endsWith("/")) return true;
        return !"en".equals(language) && reading.startsWith("[") && reading.endsWith("]");
    }
}
