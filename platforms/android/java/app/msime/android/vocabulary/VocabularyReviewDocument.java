package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Turns the shared 背单词 status into a {@link VocabularyReviewModel}.
 *
 * <p>Kept apart from the model for the same reason the statistics decoder is: `org.json` is a stub
 * in the SDK jar and throws on a host JVM, so the model can be exercised by `check-host.sh` and
 * this cannot.
 */
public final class VocabularyReviewDocument {
    private VocabularyReviewDocument() {}

    /** The model, or {@code null} when this is not a status document. */
    public static VocabularyReviewModel parse(String document) {
        if (document == null || document.isEmpty()) return null;
        try {
            return from(new JSONObject(document));
        } catch (JSONException error) {
            return null;
        }
    }

    /**
     * The model from an already-decoded status.
     *
     * <p>Returns {@code null} rather than a zeroed model for a document it does not recognise.
     * "Nothing read" and "nothing to review" say different things to whoever is reading the page,
     * and the statistics decoder beside this one makes the same distinction for the same reason.
     */
    public static VocabularyReviewModel from(JSONObject root) {
        try {
            return decode(root);
        } catch (IllegalArgumentException error) {
            return null;
        }
    }

    private static VocabularyReviewModel decode(JSONObject root) {
        if (root == null || !root.has("settings") || !root.has("queue")) return null;
        JSONObject settings = root.optJSONObject("settings");
        if (settings == null) return null;

        List<VocabularyReviewModel.Wordbook> wordbooks = new ArrayList<>();
        JSONArray books = root.optJSONArray("wordbooks");
        if (books != null) {
            for (int index = 0; index < books.length(); index++) {
                JSONObject book = books.optJSONObject(index);
                if (book == null) continue;
                String id = optionalString(book, "id", "");
                if (id.isEmpty()) continue;
                wordbooks.add(new VocabularyReviewModel.Wordbook(
                    id,
                    optionalString(book, "name", id),
                    count(book, "total"),
                    optionalBoolean(book, "builtin", false)));
            }
        }

        List<VocabularyReviewModel.Card> queue = new ArrayList<>();
        JSONArray cards = root.optJSONArray("queue");
        if (cards != null) {
            for (int index = 0; index < cards.length(); index++) {
                JSONObject card = cards.optJSONObject(index);
                if (card == null) continue;
                String word = optionalString(card, "word", "");
                // A card with no word could never be answered: the answer is keyed by it.
                if (word.isEmpty()) continue;
                queue.add(new VocabularyReviewModel.Card(
                    word,
                    optionalString(card, "phonetic", ""),
                    optionalString(card, "meaning", "")));
            }
        }

        return new VocabularyReviewModel(
            wordbooks,
            optionalString(settings, "wordbook", ""),
            count(settings, "newPerDay"),
            count(settings, "sessionLimit"),
            count(root, "due"),
            count(root, "answeredToday"),
            count(root, "introducing"),
            count(root, "remaining"),
            queue);
    }

    private static int count(JSONObject object, String key) {
        return object.has(key) ? VocabularyReviewModel.strictCount(object.opt(key)) : 0;
    }

    private static String optionalString(JSONObject object, String key, String fallback) {
        if (!object.has(key) || object.isNull(key)) return fallback;
        return VocabularyReviewModel.strictString(object.opt(key));
    }

    private static boolean optionalBoolean(JSONObject object, String key, boolean fallback) {
        if (!object.has(key) || object.isNull(key)) return fallback;
        return VocabularyReviewModel.strictBoolean(object.opt(key));
    }
}
