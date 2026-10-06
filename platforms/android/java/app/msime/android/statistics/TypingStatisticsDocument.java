package app.msime.android;

import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Turns the shared statistics document into a {@link TypingStatisticsModel}.
 *
 * <p>Kept apart from the model so the arithmetic that page depends on can be exercised without an
 * Android runtime: `org.json` is a stub in the SDK jar and cannot run on a host JVM.
 */
public final class TypingStatisticsDocument {
    private TypingStatisticsDocument() {}

    /** The model, or {@code null} when this is not a statistics document. */
    public static TypingStatisticsModel parse(String document) {
        if (document == null || document.isEmpty()) return null;
        try {
            return from(new JSONObject(document));
        } catch (JSONException error) {
            return null;
        }
    }

    /**
     * The model from an already-decoded document.
     *
     * <p>The day axis is a map keyed by `YYYY-MM-DD`, not a list. A reader that went looking for an
     * array called `daily` found nothing in every profile and reported them all as never recorded.
     */
    public static TypingStatisticsModel from(JSONObject root) {
        if (root == null || !root.has("days")) return null;
        JSONObject detail = root.optJSONObject("detail");
        JSONObject dailyDetails = root.optJSONObject("dailyDetails");
        // Absent in documents written before key counts existed; the store defaults it to empty, and so does this.
        JSONObject dailyKeyCounts = root.optJSONObject("dailyKeys");
        Map<String, Map<String, Long>> dailyKeys = new LinkedHashMap<>();
        if (dailyKeyCounts != null) {
            for (Iterator<String> keys = dailyKeyCounts.keys(); keys.hasNext();) {
                String day = keys.next();
                Map<String, Long> counts = counts(dailyKeyCounts.optJSONObject(day));
                if (!counts.isEmpty()) dailyKeys.put(day, counts);
            }
        }
        Map<String, Map<String, Long>> dailyCharacters = new LinkedHashMap<>();
        Map<String, Map<String, Long>> dailySources = new LinkedHashMap<>();
        if (dailyDetails != null) {
            for (Iterator<String> keys = dailyDetails.keys(); keys.hasNext();) {
                String day = keys.next();
                JSONObject value = dailyDetails.optJSONObject(day);
                if (value == null) continue;
                dailyCharacters.put(day, counts(value.optJSONObject("characters")));
                dailySources.put(day, counts(value.optJSONObject("sources")));
            }
        }
        return new TypingStatisticsModel(
            // The shared Rust store defaults a missing field to false. Keep old or partially
            // written documents opt-in on Android as well; showing them as enabled would expose
            // statistics the user never turned on.
            root.optBoolean("enabled", false),
            Math.max(0, KeyboardGeometry.strictLong(root.opt("total"), 0)),
            root.optString("retention", "forever"),
            counts(root.optJSONObject("days")),
            detail == null ? Map.of() : counts(detail.optJSONObject("characters")),
            detail == null ? Map.of() : counts(detail.optJSONObject("sources")),
            Map.copyOf(dailyCharacters),
            Map.copyOf(dailySources),
            Map.copyOf(dailyKeys));
    }

    private static Map<String, Long> counts(JSONObject value) {
        if (value == null) return Map.of();
        Map<String, Long> result = new LinkedHashMap<>();
        for (Iterator<String> keys = value.keys(); keys.hasNext();) {
            String key = keys.next();
            long count = KeyboardGeometry.strictLong(value.opt(key), 0);
            if (count > 0) result.put(key, count);
        }
        return Map.copyOf(result);
    }
}
