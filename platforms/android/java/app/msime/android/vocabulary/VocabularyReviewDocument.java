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
        if (root == null || !root.has("settings") || !root.has("queue")) return null;
        JSONObject settings = root.optJSONObject("settings");
        if (settings == null) return null;

        List<VocabularyReviewModel.Wordbook> wordbooks = new ArrayList<>();
        JSONArray books = root.optJSONArray("wordbooks");
        if (books != null) {
            for (int index = 0; index < books.length(); index++) {
                JSONObject book = books.optJSONObject(index);
                if (book == null) continue;
                String id = book.optString("id", "");
                if (id.isEmpty()) continue;
                wordbooks.add(new VocabularyReviewModel.Wordbook(
                    id,
                    book.optString("name", id),
                    book.optInt("total", 0),
                    book.optBoolean("builtin", false)));
            }
        }

        List<VocabularyReviewModel.Card> queue = new ArrayList<>();
        JSONArray cards = root.optJSONArray("queue");
        if (cards != null) {
            for (int index = 0; index < cards.length(); index++) {
                JSONObject card = cards.optJSONObject(index);
                if (card == null) continue;
                String word = card.optString("word", "");
                // A card with no word could never be answered: the answer is keyed by it.
                if (word.isEmpty()) continue;
                queue.add(new VocabularyReviewModel.Card(
                    word,
                    card.optString("phonetic", ""),
                    card.optString("meaning", "")));
            }
        }

        return new VocabularyReviewModel(
            wordbooks,
            settings.optString("wordbook", ""),
            settings.optInt("newPerDay", 0),
            settings.optInt("sessionLimit", 0),
            root.optInt("due", 0),
            root.optInt("answeredToday", 0),
            root.optInt("introducing", 0),
            root.optInt("remaining", 0),
            queue);
    }
}
