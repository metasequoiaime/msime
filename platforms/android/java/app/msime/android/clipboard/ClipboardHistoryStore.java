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
        if (response != null && JsonPolicy.strictTrue(response.opt("captured"))) return null;
        String reason = response == null ? "" : JsonPolicy.strictStringOrEmpty(response.opt("reason"));
        return reason;
    }

    /** Entries are identified by their text, which is how the shared store names them. */
    public void remove(String text) {
        request("remove", text, false);
    }

    public void setPinned(String text, boolean pinned) {
        request("set_pinned", text, pinned);
    }

    /**
     * 把一条历史的文字改成 `replacement`（#5971）。条目按原文找，改完留在原位，时间戳和固定状态都不变；新文字和另一条已有的历史相同时两条合并。规则都在共享存储里（`crates/client-core/src/clipboard.rs` 的 `replace`），这里只把它的答复换成 {@link ClipboardHistoryPolicy.EditResult}。
     *
     * <p>存储写不进去或文件已坏时和其他操作一样抛 {@link IllegalStateException}。
     */
    public ClipboardHistoryPolicy.EditResult replace(String text, String replacement) {
        if (text == null || text.isEmpty() || replacement == null) {
            throw new IllegalArgumentException("Clipboard edit needs the original and the new text");
        }
        JSONObject response = request("replace", text, false, replacement);
        if (response == null) throw new IllegalStateException("Clipboard history replace had no answer");
        if (JsonPolicy.strictTrue(response.opt("replaced"))) {
            return JsonPolicy.strictTrue(response.opt("merged"))
                ? ClipboardHistoryPolicy.EditResult.MERGED : ClipboardHistoryPolicy.EditResult.SAVED;
        }
        return ClipboardHistoryPolicy.editRejection(JsonPolicy.strictStringOrEmpty(response.opt("reason")));
    }

    public void clear() {
        request("clear", null, false);
    }

    /**
     * Clear without being able to stop the caller.
     *
     * <p>键盘在实时读到的偏好说开关关着时顺手清空历史（见 {@link ClipboardHistoryRetentionPolicy}）。这不是用户要求的操作，存储写不进去也不能成为不画键盘的理由。以前就是这样出的事：清空原先在 `onCreateInputView` 里执行，`IllegalStateException` 从框架的 `showWindow` 里抛出去，输入法进程退出，Android 换成了别的键盘。
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

    private JSONObject request(String operation, String text, boolean pinned) {
        return request(operation, text, pinned, null);
    }

    /** 共享入口要的请求文档：以 `operation` 区分操作的一个对象；`replacement` 只用于 `replace`，其他操作传 null。 */
    private JSONObject request(String operation, String text, boolean pinned, String replacement) {
        if (directory == null) return null;
        try {
            JSONObject action = new JSONObject().put("operation", operation);
            if (text != null) action.put("text", text);
            if ("set_pinned".equals(operation)) action.put("pinned", pinned);
            if (replacement != null) action.put("replacement", replacement);
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
                String reason = JsonPolicy.strictString(response.opt("error"));
                throw new IllegalStateException("Clipboard history " + operation
                    + " refused: " + (reason == null ? "no reason given" : reason));
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
            String text = JsonPolicy.strictString(entry.opt("text"));
            if (text == null || text.isEmpty()) continue;
            Object rawPinned = entry.opt("pinned");
            Boolean pinned = rawPinned == null || rawPinned == JSONObject.NULL
                ? Boolean.FALSE : JsonPolicy.strictBoolean(rawPinned);
            if (pinned == null) continue;
            items.add(new ClipboardHistory.Item(text,
                ClipboardHistoryPolicy.timestampValue(entry.opt("timestampMs")), pinned));
        }
        return items;
    }

}
