package app.msime.android;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Bounded host orchestration for display-only offline candidate glosses. */
public final class CandidateGlossModel {
    public static final int MAX_CANDIDATES = 4096;
    public static final int MAX_REQUEST_BYTES = 262_144;
    public static final int MAX_RESPONSE_BYTES = 1_048_576;

    public record Result(long generation, String translations) {
        public Result {
            if (generation < 0 || translations == null)
                throw new IllegalArgumentException("Invalid candidate gloss result");
        }
    }

    private CandidateGlossModel() {}

    /** Copy only stable display fields from one complete Engine candidate generation. */
    public static String request(long generation, JSONArray candidates) throws JSONException {
        return request(generation, candidates, null);
    }

    /** The same request answered from the offline dictionary for {@code targetLanguage}; null asks for the English gloss. */
    public static String request(long generation, JSONArray candidates, String targetLanguage)
            throws JSONException {
        if (targetLanguage != null
                && !CandidateTranslationPolicy.OFFLINE_GLOSS_LANGUAGES.contains(targetLanguage))
            throw new IllegalArgumentException("Invalid candidate gloss language");
        if (generation < 0 || candidates == null || candidates.length() == 0
                || candidates.length() > MAX_CANDIDATES)
            throw new IllegalArgumentException("Invalid candidate gloss request");
        JSONArray copied = new JSONArray();
        for (int index = 0; index < candidates.length(); index++) {
            JSONObject candidate = candidates.optJSONObject(index);
            if (candidate == null) throw new IllegalArgumentException("Invalid candidate entry");
            String text = CandidateGlossPolicy.strictString(candidate.opt("text"));
            long source = CandidateGlossPolicy.strictInteger(candidate.opt("source"));
            if (!CandidateGlossPolicy.validEntry(text) || source < 0 || source > 255)
                throw new IllegalArgumentException("Invalid candidate entry");
            copied.put(new JSONObject().put("text", text).put("source", (int) source));
        }
        JSONObject envelope = new JSONObject().put("generation", generation)
            .put("candidates", copied);
        if (targetLanguage != null) envelope.put("target_language", targetLanguage);
        String request = envelope.toString();
        if (TextPolicy.utf8Length(request) > MAX_REQUEST_BYTES)
            throw new IllegalArgumentException("Candidate gloss request is too large");
        return request;
    }

    /** Validate the native envelope before returning an apply_translations payload. */
    public static Result decode(String response) throws JSONException {
        if (response == null
                || TextPolicy.utf8Length(response) > MAX_RESPONSE_BYTES)
            throw new IllegalArgumentException("Candidate gloss response is too large");
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new JSONException("Candidate gloss failed");
        JSONObject value = envelope.getJSONObject("value");
        long generation = CandidateGlossPolicy.strictInteger(value.get("generation"));
        JSONArray entries = value.getJSONArray("translations");
        if (generation < 0 || entries.length() > MAX_CANDIDATES)
            throw new IllegalArgumentException("Invalid candidate gloss response");
        JSONArray copied = new JSONArray();
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.optJSONObject(index);
            if (entry == null) throw new IllegalArgumentException("Invalid candidate gloss entry");
            String text = CandidateGlossPolicy.strictString(entry.opt("text"));
            String translation = CandidateGlossPolicy.strictString(entry.opt("translation"));
            if (!CandidateGlossPolicy.validEntry(text)
                    || !CandidateGlossPolicy.validEntry(translation))
                throw new IllegalArgumentException("Invalid candidate gloss entry");
            copied.put(new JSONObject().put("text", text).put("translation", translation));
        }
        String payload = copied.toString();
        if (TextPolicy.utf8Length(payload) > MAX_RESPONSE_BYTES)
            throw new IllegalArgumentException("Candidate gloss payload is too large");
        return new Result(generation, payload);
    }

}
