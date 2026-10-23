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
 * <p>历史记录的排序、所选容量、淘汰规则、固定项不可淘汰规则和磁盘格式都由共享存储决定；此类只把
 * 共享存储的结果转换成键盘绘制所需的模型。
 */
public final class ClipboardHistoryStore {
    private static final String TAG = "MSIMEClipboard";
    private final String directory;
    private int limit = ClipboardHistoryPolicy.DEFAULT_LIMIT;

    /**
     * @param directory the host's own data directory; the shared store keeps its file beneath it
     */
    public ClipboardHistoryStore(File directory) {
        this.directory = directory == null ? null : directory.getAbsolutePath();
    }

    public record Category(String id, String name) {}
    public record Catalog(List<Category> categories, int maxCategories, int maxNameCharacters) {}
    private List<Category> categories = List.of();

    public List<Category> categories() { return categories; }

    public Catalog catalog() {
        JSONObject value = request("categories", null, false);
        if (value == null) throw new IllegalStateException("Clipboard categories unavailable");
        return new Catalog(parseCategories(value), value.optInt("maxCategories"), value.optInt("maxNameCharacters"));
    }

    public boolean addCategory(String name) { return categoryAction("add_category", null, name, null); }
    public boolean renameCategory(String id, String name) { return categoryAction("rename_category", id, name, null); }
    public boolean removeCategory(String id) { return categoryAction("remove_category", id, null, null); }
    public boolean setCategory(String text, String id) { return categoryAction("set_category", id, null, text); }

    private boolean categoryAction(String operation, String id, String name, String text) {
        try {
            JSONObject action = new JSONObject().put("operation", operation);
            if (id != null) action.put("id", id);
            if (name != null) action.put("name", name);
            if (text != null) action.put("text", text);
            JSONObject value = send(action);
            return value != null && JsonPolicy.strictTrue(value.opt("updated"));
        } catch (JSONException | LinkageError error) {
            throw new IllegalStateException("Clipboard category operation unavailable", error);
        }
    }

    private List<Category> parseCategories(JSONObject value) {
        JSONArray values = value.optJSONArray("categories");
        if (values == null) throw new IllegalStateException("Clipboard categories unavailable");
        List<Category> result = new ArrayList<>();
        for (int index = 0; index < values.length(); index++) {
            JSONObject category = values.optJSONObject(index);
            if (category == null || !(category.opt("id") instanceof String)
                || !(category.opt("name") instanceof String)) throw new IllegalStateException("Invalid clipboard category");
            result.add(new Category(category.optString("id"), category.optString("name")));
        }
        return List.copyOf(result);
    }

    public List<ClipboardHistory.Item> load() {
        return entries(request("load", null, false));
    }

    public int limit() {
        return limit;
    }

    /**
     * Add one entry.
     *
     * <p>Returns null when it was stored, and the shared store's own reason when it was not.
     * Deliberately not annotated: this file is compiled by the JVM smoke stage, which skips any
     * source that imports androidx, and being excluded from that compile is worse than a
     * contract stated in prose. The
     * reason used to be discarded, so a refusal for a control character or an over-long text
     * 结果曾被显示成所选容量的记录全部固定。
     */
    public String add(String text) {
        if (!ClipboardHistoryPolicy.hasText(text)) {
            throw new IllegalArgumentException("Clipboard has no usable text");
        }
        JSONObject response = request("capture", text, false);
        updateLimit(response);
        if (response != null && JsonPolicy.strictTrue(response.opt("captured"))) return null;
        String reason = response == null ? "" : JsonPolicy.strictStringOrEmpty(response.opt("reason"));
        return reason;
    }

    /** Entries are identified by their text, which is how the shared store names them. */
    public void remove(String text) {
        request("remove", text, false);
    }

    /** 原子删除多条记录，并交回删除前的完整快照供撤销。 */
    public List<ClipboardHistory.Item> removeMany(List<String> texts) {
        if (texts == null || texts.isEmpty()) return List.of();
        JSONObject response = request("remove_many", texts);
        return entriesArray(response == null ? null : response.optJSONArray("removed"));
    }

    /** 原子恢复删除前的快照，保留时间戳和固定状态。 */
    public void restoreMany(List<ClipboardHistory.Item> items) {
        if (items == null || items.isEmpty()) return;
        try {
            JSONArray entries = new JSONArray();
            for (ClipboardHistory.Item item : items) entries.put(new JSONObject()
                .put("text", item.text()).put("timestampMs", item.timestamp())
                .put("pinned", item.pinned()).put("categoryId", item.categoryId()));
            request("restore_many", entries);
        } catch (JSONException error) {
            throw new IllegalArgumentException("Clipboard history restore entries are invalid", error);
        }
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

    /** 全文格式由共享 Rust 生成，Android 只负责把快照写到用户选定的位置。 */
    public record Export(String text, int count) {}

    public Export exportText() {
        JSONObject value = request("export_text", null, false);
        if (value == null || !(value.opt("text") instanceof String))
            throw new IllegalStateException("Clipboard export had no answer");
        return new Export(value.optString("text"), value.optInt("count", 0));
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

    private JSONObject request(String operation, List<String> texts) {
        if (directory == null) return null;
        try {
            JSONObject action = new JSONObject().put("operation", operation).put("texts", new JSONArray(texts));
            return send(action);
        } catch (JSONException | LinkageError error) {
            throw new IllegalStateException("Clipboard history " + operation + " could not reach the shared store", error);
        }
    }

    private JSONObject request(String operation, JSONArray entries) {
        if (directory == null) return null;
        try {
            JSONObject action = new JSONObject().put("operation", operation).put("entries", entries);
            return send(action);
        } catch (JSONException | LinkageError error) {
            throw new IllegalStateException("Clipboard history " + operation + " could not reach the shared store", error);
        }
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
            return send(action);
        } catch (JSONException | LinkageError error) {
            throw new IllegalStateException(
                "Clipboard history " + operation + " could not reach the shared store", error);
        }
    }

    private JSONObject send(JSONObject action) throws JSONException {
        JSONObject payload = new JSONObject().put("directory", directory).put("action", action);
        JSONObject response = new JSONObject(NativeClient.mobileClipboardHistory(payload.toString()));
        String operation = action.optString("operation", "unknown");
        if (!JsonPolicy.strictTrue(response.opt("ok"))) {
                // Name the operation and carry the shared entry's own reason. Without them a
                // refusal reaches the log as one indistinguishable sentence, which is how an
                // Android-only file-locking failure read as "the clipboard is broken somehow"
                // for as long as it did.
                String reason = JsonPolicy.strictString(response.opt("error"));
            throw new IllegalStateException("Clipboard history " + operation
                + " refused: " + (reason == null ? "no reason given" : reason));
        }
        return JsonPolicy.objectValue(response);
    }

    private List<ClipboardHistory.Item> entriesArray(JSONArray values) {
        List<ClipboardHistory.Item> items = new ArrayList<>();
        if (values == null) return items;
        for (int index = 0; index < values.length(); index++) {
            JSONObject entry = values.optJSONObject(index);
            if (entry == null) continue;
            String text = JsonPolicy.strictString(entry.opt("text"));
            Boolean pinned = JsonPolicy.strictBoolean(entry.opt("pinned"));
            if (text != null && pinned != null)
                items.add(new ClipboardHistory.Item(text,
                    ClipboardHistoryPolicy.timestampValue(entry.opt("timestampMs")), pinned,
                    JsonPolicy.strictString(entry.opt("categoryId"))));
        }
        return items;
    }

    private List<ClipboardHistory.Item> entries(JSONObject value) {
        updateLimit(value);
        if (value == null) throw new IllegalStateException("Clipboard history unavailable");
        categories = parseCategories(value);
        List<ClipboardHistory.Item> items = new ArrayList<>(limit);
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
                ClipboardHistoryPolicy.timestampValue(entry.opt("timestampMs")), pinned,
                    JsonPolicy.strictString(entry.opt("categoryId"))));
        }
        return items;
    }

    private void updateLimit(JSONObject value) {
        if (value == null) return;
        int reported = NumberPolicy.strictInt(value.opt("limit"), ClipboardHistoryPolicy.DEFAULT_LIMIT);
        if (ClipboardHistoryPolicy.isSupportedLimit(reported)) limit = reported;
    }

}
