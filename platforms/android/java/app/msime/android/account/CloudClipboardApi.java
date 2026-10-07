package app.msime.android;

import app.msime.android.clipboard.CloudClipboardTextPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「我的 → 云剪贴板」页的接口：读列表、开关、保留时长、置顶、删除与清空。
 *
 * <p>条目在服务端是明文保存的（经 HTTPS 传输，关闭云剪贴板即全部删除），客户端不做加密，所以页面不写「端到端加密」。只有真实账号有云剪贴板。服务端最多保存 50 条，单条不超过 4000 个 UTF-16 单元；保留时长 0 表示一直保留。列表响应里的 `retention_days`、条目的 `pinned` 与 `device` 是新字段，旧服务端没有时分别按 0、false、空字符串处理。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class CloudClipboardApi {
    public static final String PATH = "/v1/users/me/clipboard";
    public static final int MAX_ITEMS = 50;
    /** 保留时长的可选值（天），0 表示一直保留。 */
    public static final List<Integer> RETENTION_DAYS = List.of(1, 7, 30, 0);
    private static final Pattern ID = Pattern.compile("[0-9a-f]{64}");

    /** 一条记录。`device` 是写入时的设备名，未知时为空字符串。 */
    public record Item(String id, String text, String updatedAt, boolean pinned, String device) {}

    /** 一次读取的结果。 */
    public record Page(boolean enabled, int retentionDays, List<Item> items) {}

    private final CloudApi api;

    public CloudClipboardApi(CloudApi api) {
        this.api = api;
    }

    public static boolean validId(String id) {
        return id != null && ID.matcher(id).matches();
    }

    public static boolean validRetention(int days) {
        return RETENTION_DAYS.contains(days);
    }

    /** 置顶的排在前面，其余保持服务端给的顺序（新的在前）。 */
    public static List<Item> ordered(List<Item> items) {
        int capacity = items == null ? 0 : items.size();
        List<Item> ordered = new ArrayList<>(capacity);
        for (Item item : items) if (item.pinned()) ordered.add(item);
        for (Item item : items) if (!item.pinned()) ordered.add(item);
        return ordered;
    }

    public Page load() throws CloudApi.Failure {
        JSONObject response = api.json("GET", PATH + "?q=", null, CloudApi.Auth.ACCOUNT);
        try {
            JSONArray values = response.optJSONArray("items");
            Object enabled = response.opt("enabled");
            if (values == null || values.length() > MAX_ITEMS || !(enabled instanceof Boolean)) throw invalid();
            Integer rawRetention = strictInteger(response.opt("retention_days"));
            int retention = rawRetention == null ? 0 : rawRetention;
            if (!validRetention(retention)) retention = 0;
            List<Item> items = new ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                JSONObject value = values.getJSONObject(index);
                String id = strictString(value.opt("id"));
                String text = strictString(value.opt("text"));
                String updated = strictString(value.opt("updated_at"));
                Object rawDevice = value.opt("device");
                String device = rawDevice == null || rawDevice == JSONObject.NULL
                    ? "" : strictString(rawDevice);
                Object rawPinned = value.opt("pinned");
                Boolean pinned = rawPinned == null || rawPinned == JSONObject.NULL
                    ? Boolean.FALSE : strictBoolean(rawPinned);
                if (pinned == null || id == null || text == null || updated == null || device == null) {
                    throw invalid();
                }
                if (!validId(id) || !CloudClipboardTextPolicy.valid(text) || updated.isEmpty()
                        || TextPolicy.utf8Length(updated) > 128 || TextPolicy.hasControl(updated)
                        || !TextPolicy.validUnicode(updated)) throw invalid();
                if (TextPolicy.utf8Length(device) > 128 || TextPolicy.hasControl(device)
                        || !TextPolicy.validUnicode(device)) device = "";
                items.add(new Item(id, text, updated, pinned, device));
            }
            return new Page((Boolean) enabled, retention, ordered(items));
        } catch (JSONException malformed) {
            throw invalid();
        }
    }

    public void setEnabled(boolean enabled) throws CloudApi.Failure {
        api.json("PUT", PATH + "/settings", object("enabled", enabled), CloudApi.Auth.ACCOUNT);
    }

    public void setRetention(int days) throws CloudApi.Failure {
        if (!validRetention(days)) throw new IllegalArgumentException("unsupported retention " + days);
        api.json("PUT", PATH + "/retention", object("days", days), CloudApi.Auth.ACCOUNT);
    }

    public void setPinned(String id, boolean pinned) throws CloudApi.Failure {
        if (!validId(id)) throw new IllegalArgumentException("invalid clipboard id");
        api.json("PUT", PATH + "/" + id + "/pin", object("pinned", pinned), CloudApi.Auth.ACCOUNT);
    }

    public void delete(String id) throws CloudApi.Failure {
        if (!validId(id)) throw new IllegalArgumentException("invalid clipboard id");
        api.send("DELETE", PATH + "/" + id, null, CloudApi.Auth.ACCOUNT);
    }

    /** 清空全部记录（开关保持不变）。 */
    public void clear() throws CloudApi.Failure {
        api.send("DELETE", PATH, null, CloudApi.Auth.ACCOUNT);
    }

    private static JSONObject object(String key, Object value) {
        try {
            return new JSONObject().put(key, value);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static CloudApi.Failure invalid() {
        return new CloudApi.Failure(500, "invalid_response", "invalid clipboard response", 0);
    }

    /** org.json's optBoolean accepts string values; server response fields must keep their JSON type. */
    static Boolean strictBoolean(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
    }

    /** org.json's optString coerces numbers and booleans; response text fields must stay strings. */
    static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }

    /** Retention days must be a JSON integer; reject strings and fractional numbers. */
    static Integer strictInteger(Object value) {
        if (value instanceof Integer integer) return integer;
        if (value instanceof Long longValue
                && longValue >= Integer.MIN_VALUE && longValue <= Integer.MAX_VALUE) {
            return longValue.intValue();
        }
        return null;
    }
}
