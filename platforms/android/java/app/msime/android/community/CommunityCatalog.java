package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
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
    private static final int TIMEOUT_MILLIS = 30_000;

    /** One catalogue entry, flattened to what a list row shows. */
    public record Item(String id, CommunityRequest.Kind kind, String name, String description,
                       String author, int saves, int ratingCount, double ratingAverage,
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
                    new String(readBounded(input), StandardCharsets.UTF_8)));
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
        if (values == null) return new Page(List.of(), false, "");
        if (exceedsPageLimit(values.length())) {
            return new Page(List.of(), false, CommunityRequest.message(null, 500));
        }
        List<Item> items = new ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            JSONObject value = values.optJSONObject(index);
            if (value == null) continue;
            String id = value.optString("id", "");
            String name = value.optString("name", "").trim();
            if (id.isEmpty() || name.isEmpty()) continue;
            items.add(new Item(id, kind, name, value.optString("description", "").trim(),
                value.optString("author", "").trim(),
                value.optInt("saves", value.optInt("downloads", 0)),
                value.optInt("rating_count", 0), value.optDouble("rating_average", 0),
                kind == CommunityRequest.Kind.SKIN ? value.optJSONObject("design")
                    : value.optJSONObject("content")));
        }
        return new Page(List.copyOf(items), root.optBoolean("has_more", false), "");
    }

    /** A page longer than the one the client asked for is a backend fault, not more results to show. Kept apart from parse so the JVM smoke can check it: the smokes run against android.jar, whose org.json classes are stubs that throw. */
    private static boolean exceedsPageLimit(int length) {
        return length > CommunityRequest.PAGE_SIZE;
    }

    /** The backend's own name for a failure, so the reader is told the specific thing. */
    private static String errorCode(InputStream errors) {
        if (errors == null) return "";
        try (InputStream input = errors) {
            JSONObject root = new JSONObject(
                new String(readBounded(input), StandardCharsets.UTF_8));
            JSONObject error = root.optJSONObject("error");
            return error == null ? "" : error.optString("code", "");
        } catch (Exception error) {
            return "";
        }
    }

    private static byte[] readBounded(InputStream input) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > MAX_RESPONSE_BYTES)
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
