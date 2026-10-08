package app.msime.android;

import android.util.Log;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * The clipboard history, in the one file every mobile host shares.
 *
 * <p>This used to be a private `SharedPreferences` document with its own ordering, eviction and
 * pinning rules. The settings page reads the shared store — the same file iOS and HarmonyOS use —
 * so the keyboard and the settings page were showing two different histories: text copied here
 * never appeared there, and deleting an entry there left it on the keyboard.
 *
 * <p>Nothing about the history is decided here now. Ordering, the fifty-entry limit, eviction, the
 * pinned-entries-cannot-be-evicted rule and the on-disk format all belong to the shared store; this
 * class turns its answers into the model this keyboard draws.
 */
public final class ClipboardHistoryStore {
    private static final String TAG = "MSIMEClipboard";
    private final String directory;

    /**
     * @param directory the host's own data directory; the shared store keeps its file beneath it
     */
    public ClipboardHistoryStore(File directory) {
        this.directory = directory == null ? null : directory.getAbsolutePath();
    }

    public List<ClipboardHistory.Item> load() {
        return entries(request("load", null, false));
    }

    /**
     * Add one entry.
     *
     * <p>Returns null when it was stored, and the shared store's own reason when it was not.
     * Deliberately not annotated: this file is compiled by the JVM smoke stage, which skips any
     * source that imports androidx, and being excluded from that compile is worse than a
     * contract stated in prose. The
     * reason used to be discarded, so a refusal for a control character or an over-long text
     * reached the user as 「50 条历史均已固定」 - the one refusal it could not have been.
     */
    public String add(String text) {
        if (!ClipboardHistoryPolicy.hasText(text)) {
            throw new IllegalArgumentException("Clipboard has no usable text");
        }
        JSONObject response = request("capture", text, false);
        if (response != null && Boolean.TRUE.equals(
                ClipboardHistoryPolicy.strictBoolean(response.opt("captured")))) return null;
        String reason = response == null || response.isNull("reason")
            ? "" : response.optString("reason", "");
        return reason;
    }

    /** Entries are identified by their text, which is how the shared store names them. */
    public void remove(String text) {
        request("remove", text, false);
    }

    public void setPinned(String text, boolean pinned) {
        request("set_pinned", text, pinned);
    }

    public void clear() {
        request("clear", null, false);
    }

    /**
     * Clear without being able to stop the caller.
     *
     * <p>The keyboard drops the history as housekeeping - when the preference goes off, and again
     * on every `onCreateInputView` while it is off. None of those is the user asking for anything,
     * and a store that cannot be written is not a reason to refuse to draw a keyboard. It was:
     * `onCreateInputView` threw `IllegalStateException` out of the framework's `showWindow`, so the
     * input method died and Android fell back to another keyboard.
     *
     * <p>The `清空` button keeps {@link #clear()}: there the user asked, and silence would be a lie.
     */
    public void clearQuietly() {
        try {
            clear();
        } catch (IllegalStateException error) {
            Log.w(TAG, "Clipboard history could not be cleared", error);
        }
    }

    /** The request document the shared entry takes: one internally tagged `operation`. */
    private JSONObject request(String operation, String text, boolean pinned) {
        if (directory == null) return null;
        try {
            JSONObject action = new JSONObject().put("operation", operation);
            if (text != null) action.put("text", text);
            if ("set_pinned".equals(operation)) action.put("pinned", pinned);
            JSONObject payload = new JSONObject()
                .put("directory", directory)
                .put("action", action);
            JSONObject response = new JSONObject(
                NativeClient.mobileClipboardHistory(payload.toString()));
            if (!JsonPolicy.strictTrue(response.opt("ok"))) {
                // Name the operation and carry the shared entry's own reason. Without them a
                // refusal reaches the log as one indistinguishable sentence, which is how an
                // Android-only file-locking failure read as "the clipboard is broken somehow"
                // for as long as it did.
                throw new IllegalStateException("Clipboard history " + operation
                    + " refused: " + response.optString("error", "no reason given"));
            }
            return response.optJSONObject("value");
        } catch (JSONException | LinkageError error) {
            throw new IllegalStateException(
                "Clipboard history " + operation + " could not reach the shared store", error);
        }
    }

    private static List<ClipboardHistory.Item> entries(JSONObject value) {
        List<ClipboardHistory.Item> items = new ArrayList<>(ClipboardHistoryPolicy.LIMIT);
        JSONArray entries = value == null ? null : value.optJSONArray("entries");
        if (entries == null) return items;
        for (int index = 0; index < entries.length(); index++) {
            JSONObject entry = entries.optJSONObject(index);
            if (entry == null) continue;
            String text = ClipboardHistoryPolicy.strictString(entry.opt("text"));
            if (text == null || text.isEmpty()) continue;
            Object rawPinned = entry.opt("pinned");
            Boolean pinned = rawPinned == null || rawPinned == JSONObject.NULL
                ? Boolean.FALSE : ClipboardHistoryPolicy.strictBoolean(rawPinned);
            if (pinned == null) continue;
            items.add(new ClipboardHistory.Item(text,
                ClipboardHistoryPolicy.timestampValue(entry.opt("timestampMs")), pinned));
        }
        return items;
    }

    /** Shared-store response flags must remain JSON booleans; org.json otherwise coerces strings. */
    static Boolean strictBoolean(Object value) {
        return ClipboardHistoryPolicy.strictBoolean(value);
    }

}
