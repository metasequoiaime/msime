package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 读社区目录：皮肤、词包和回复模板。
 *
 * <p>Read-only on purpose. Publishing, rating and deleting are the operations that need a real
 * signed-in account, and this host only has the keyboard's anonymous identity -- offering a publish
 * button that always answers "请先登录" would be worse than not offering one. Downloading a skin,
 * which is what someone opens this tab to do, needs nothing more than the anonymous token.
 *
 * <p>Every call blocks on the network and must not run on the main thread.
 */
public final class CommunityCatalog {
    private static final String ORIGIN = "https://api.msime.app";
    private static final int MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
    private static final int MAX_RESOURCE_RESPONSE_BYTES = 48 * 1024 * 1024;
    private static final int TIMEOUT_MILLIS = 30_000;
    private static final long MAX_JAVASCRIPT_INTEGER = 9_007_199_254_740_991L;
    private static final int MAX_NAME_CHARACTERS = 32;
    private static final int MAX_DESCRIPTION_CHARACTERS = 280;
    private static final int MAX_AUTHOR_CHARACTERS = 128;

    /** One catalogue entry, flattened to what a list row shows. */
    public record Item(String id, CommunityRequest.Kind kind, String name, String description,
                       String author, long saves, long ratingCount, double ratingAverage,
                       JSONObject payload) {}

    /** A page of results, or a failure the caller can show verbatim. */
    public record Page(List<Item> items, boolean hasMore, String failure) {
        public boolean failed() { return !failure.isEmpty(); }
    }

    private final Context context;

    public CommunityCatalog(Context context) {
        this.context = context.getApplicationContext();
    }

    /** One page of the catalogue. Never throws: a failure is a page that says why. */
    public Page list(CommunityRequest.Kind kind, String search, int offset) {
        // 目录本身是公开的：不带令牌也能读到完整列表，令牌只决定 owned / my_rating 这些跟人
        // 有关的字段。把它当成硬前提，就会在登录端点被限流（429）或暂时关闭时，把一页本来读得到
        // 的作品报成「连不上社区」——那句话既不对，也让人去查一个没有问题的网络。
        String token = null;
        try {
            token = new BackendAnonymousAccount(context).accessToken();
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Anonymous identity unavailable; listing anyway",
                error);
        }
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(
                ORIGIN + CommunityRequest.path(kind, "", search, offset)).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("GET");
            connection.setConnectTimeout(TIMEOUT_MILLIS);
            connection.setReadTimeout(TIMEOUT_MILLIS);
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("User-Agent", "MSIME/Android");
            if (token != null) connection.setRequestProperty("Authorization", "Bearer " + token);
            int status = connection.getResponseCode();
            if (status != 200) {
                String code = errorCode(connection.getErrorStream());
                return new Page(List.of(), false, CommunityRequest.message(code, status));
            }
            try (InputStream input = connection.getInputStream()) {
                return parse(kind, new JSONObject(
                    new String(readBounded(input, maximumResponseBytes(kind)), StandardCharsets.UTF_8)));
            }
        } catch (Exception | LinkageError error) {
            // 说出是哪一步断的。界面上仍然只有那一句，但把原因扔掉，下一次就还得从头猜。
            android.util.Log.w("MSIMECommunity", "Catalogue request failed", error);
            return new Page(List.of(), false, CommunityRequest.message(null, 0));
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private static Page parse(CommunityRequest.Kind kind, JSONObject root) {
        JSONArray values = root.optJSONArray(
            kind == CommunityRequest.Kind.SKIN ? "skins" : "items");
        if (values == null) return new Page(List.of(), false, CommunityRequest.message(null, 500));
        boolean hasMore = root.optBoolean("has_more", false);
        List<Item> items = new ArrayList<>(values.length());
        Set<String> ids = new HashSet<>();
        for (int index = 0; index < values.length(); index++) {
            JSONObject value = values.optJSONObject(index);
            if (value == null) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            String id = value.optString("id", "");
            String name = value.optString("name", "").trim();
            JSONObject payload = kind == CommunityRequest.Kind.SKIN
                ? value.optJSONObject("design") : value.optJSONObject("content");
            Long saves = count(value, "saves", kind == CommunityRequest.Kind.SKIN ? "downloads" : null);
            Long ratings = count(value, "rating_count", null);
            Double average = decimal(value, "rating_average");
            if (saves == null || ratings == null || average == null) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            Item item = new Item(id, kind, name, value.optString("description", "").trim(),
                value.optString("author", "").trim(),
                saves, ratings, average, payload);
            if (!validItem(item, kind)) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            if (!ids.add(id)) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            items.add(item);
        }
        if (invalidPage(values.length(), items.size(), hasMore)) {
            return new Page(List.of(), false, CommunityRequest.message(null, 500));
        }
        return new Page(List.copyOf(items), hasMore, "");
    }

    /** A malformed page is a backend fault, not more results to show. Kept apart from parse so the JVM smoke can check it: the smokes run against android.jar, whose org.json classes are stubs that throw. */
    static boolean invalidPage(int rawLength, int validLength, boolean hasMore) {
        return rawLength > CommunityRequest.PAGE_SIZE
            || validLength != rawLength
            || (hasMore && validLength == 0);
    }

    static boolean validItem(Item item, CommunityRequest.Kind kind) {
        if (item == null || item.kind() != kind || item.payload() == null
                || !validUuid(item.id()) || !validName(item.name(), MAX_NAME_CHARACTERS)
                || !validDescription(item.description()) || !validName(item.author(), MAX_AUTHOR_CHARACTERS)
                || item.saves() < 0 || item.saves() > MAX_JAVASCRIPT_INTEGER
                || item.ratingCount() < 0 || item.ratingCount() > MAX_JAVASCRIPT_INTEGER
                || item.ratingAverage() < 0 || item.ratingAverage() > 5
                || Double.isNaN(item.ratingAverage()) || Double.isInfinite(item.ratingAverage())) {
            return false;
        }
        return item.ratingCount() != 0 || item.ratingAverage() == 0;
    }

    private static boolean validUuid(String value) {
        if (value == null) return false;
        try {
            return UUID.fromString(value).toString().equalsIgnoreCase(value);
        } catch (IllegalArgumentException error) {
            return false;
        }
    }

    private static boolean validName(String value, int maximum) {
        return value != null && !value.isEmpty() && value.trim().equals(value)
            && value.codePointCount(0, value.length()) <= maximum && !hasDisallowedControl(value, false);
    }

    private static boolean validDescription(String value) {
        return value != null && value.codePointCount(0, value.length()) <= MAX_DESCRIPTION_CHARACTERS
            && !hasDisallowedControl(value, true);
    }

    private static boolean hasDisallowedControl(String value, boolean multiline) {
        for (int index = 0; index < value.length();) {
            int codePoint = value.codePointAt(index);
            if (Character.isISOControl(codePoint)
                    && !(multiline && (codePoint == '\n' || codePoint == '\t'))) return true;
            index += Character.charCount(codePoint);
        }
        return false;
    }

    private static Long count(JSONObject value, String primary, String fallback) {
        Object raw = value.opt(primary);
        if ((raw == null || raw == JSONObject.NULL) && fallback != null) raw = value.opt(fallback);
        if (raw == null || raw == JSONObject.NULL) return 0L;
        if (!(raw instanceof Number number)) return null;
        double decimal = number.doubleValue();
        long integer = number.longValue();
        return Double.isFinite(decimal) && decimal >= 0 && decimal == integer
            && integer <= MAX_JAVASCRIPT_INTEGER ? integer : null;
    }

    private static Double decimal(JSONObject value, String key) {
        Object raw = value.opt(key);
        if (raw == null || raw == JSONObject.NULL) return 0.0;
        if (!(raw instanceof Number number)) return null;
        double result = number.doubleValue();
        return Double.isFinite(result) ? result : null;
    }

    /** The backend's own name for a failure, so the reader is told the specific thing. */
    private static String errorCode(InputStream errors) {
        if (errors == null) return "";
        try (InputStream input = errors) {
            JSONObject root = new JSONObject(
                new String(readBounded(input, MAX_RESPONSE_BYTES), StandardCharsets.UTF_8));
            JSONObject error = root.optJSONObject("error");
            return error == null ? "" : error.optString("code", "");
        } catch (Exception error) {
            return "";
        }
    }

    static int maximumResponseBytes(CommunityRequest.Kind kind) {
        return kind == CommunityRequest.Kind.SKIN ? MAX_RESPONSE_BYTES : MAX_RESOURCE_RESPONSE_BYTES;
    }

    private static byte[] readBounded(InputStream input, int maximumBytes) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > maximumBytes)
                throw new IllegalStateException("community response too large");
            output.write(buffer, 0, count);
        }
        return output.toByteArray();
    }

    /**
     * Save a downloaded skin design into the library the keyboard's skin picker reads.
     *
     * @return the failure to show, or an empty string on success
     */
    public String install(java.nio.file.Path preferencesDirectory, Item item) {
        if (item.kind() != CommunityRequest.Kind.SKIN || item.payload() == null) {
            return "这类作品还不能从这里保存。";
        }
        try {
            return CustomSkinLibrary.add(preferencesDirectory, item.id(), item.name(),
                item.payload()) ? "" : "皮肤库已满，请先在键盘里删掉一些。";
        } catch (java.io.IOException | RuntimeException error) {
            return "保存失败，请稍后重试。";
        }
    }
}
