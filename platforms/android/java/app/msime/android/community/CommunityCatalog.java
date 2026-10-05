package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.UUID;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 读社区目录：皮肤、词包、回复模板和短语包。
 *
 * <p>Read-only on purpose. Publishing, rating and deleting are the operations that need a real
 * signed-in account, and this host only has the keyboard's anonymous identity -- offering a publish
 * button that always answers "请先登录" would be worse than not offering one. Downloading a skin,
 * which is what someone opens this tab to do, needs nothing more than the anonymous token. So does
 * reporting an entry to the moderators, the one write this host offers.
 *
 * <p>另一个写操作是作者修改自己皮肤的分类：只有登录了水杉账号、且服务端说这款是你的（`owned`）时才会出现，用的是账号令牌。
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

    /**
     * One catalogue entry, flattened to what a list row shows.
     *
     * <p>`category` 只有皮肤才有，词库、回复和短语为 null。`owned` 是服务端按请求所带令牌判断的「这是你发布的」。`downloads` 是服务端的下载计数，界面上读作「使用次数」。`raw` 是这个条目的原始 JSON，安装时原样交给本机存储（例如短语包交给常用语的导入），解析出的字段只用于展示。
     */
    public record Item(String id, CommunityRequest.Kind kind, String name, String description,
                       String author, long saves, long ratingCount, double ratingAverage,
                       JSONObject payload, CommunityRequest.Category category, boolean owned,
                       long downloads, JSONObject raw) {}

    /** 修改分类的结果：成功时是改过之后的条目，失败时是可以原样展示的原因。 */
    public record Update(Item item, String failure) {
        public boolean failed() { return item == null; }
    }

    /** A page of results, or a failure the caller can show verbatim. */
    public record Page(List<Item> items, boolean hasMore, String failure) {
        public boolean failed() { return !failure.isEmpty(); }
    }

    private final Context context;
    private record PageResponse(Page page, int status) {}
    private record ListingToken(String value, boolean anonymous) {}

    public CommunityCatalog(Context context) {
        this.context = context.getApplicationContext();
    }

    /**
     * One page of the catalogue. Never throws: a failure is a page that says why.
     *
     * @param category 只列这一类皮肤；null 列出全部
     */
    public Page list(CommunityRequest.Kind kind, String search, int offset,
            CommunityRequest.Category category) {
        // 目录本身是公开的：不带令牌也能读到完整列表，令牌只决定 owned / my_rating 这些跟人
        // 有关的字段。把它当成硬前提，就会在登录端点被限流（429）或暂时关闭时，把一页本来读得到
        // 的作品报成「连不上社区」——那句话既不对，也让人去查一个没有问题的网络。
        ListingToken selected = listingToken();
        String token = selected.value();
        for (int attempt = 0; ; attempt++) {
            PageResponse response = requestPage(kind, search, offset, category, token);
            if (!shouldRetryListing(response.status(), token, attempt)) return response.page();
            try {
                String fresh = selected.anonymous()
                    ? new BackendAnonymousAccount(context).accessToken(token)
                    : new BackendAccount(context).currentAccessToken(token);
                if (fresh.isEmpty()) return response.page();
                token = fresh;
            } catch (Exception | LinkageError error) {
                android.util.Log.i("MSIMECommunity", "Account refresh for catalogue failed", error);
                return response.page();
            }
        }
    }

    /** 登录令牌被服务端拒绝时只允许刷新并重试一次，避免重复提交或循环请求。 */
    static boolean shouldRetryListing(int status, String token, int attempt) {
        return status == 401 && token != null && !token.isEmpty() && attempt == 0;
    }

    private PageResponse requestPage(CommunityRequest.Kind kind, String search, int offset,
            CommunityRequest.Category category, String token) {
        HttpsURLConnection connection = null;
        try {
            connection = (HttpsURLConnection) new URL(ORIGIN
                + CommunityRequest.path(kind, "", search, offset, category)).openConnection();
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
                return new PageResponse(
                    new Page(List.of(), false, CommunityRequest.message(code, status)), status);
            }
            try (InputStream input = connection.getInputStream()) {
                return new PageResponse(parse(kind, new JSONObject(
                    new String(readBounded(input, maximumResponseBytes(kind)), StandardCharsets.UTF_8))), 200);
            }
        } catch (Exception | LinkageError error) {
            // 说出是哪一步断的。界面上仍然只有那一句，但把原因扔掉，下一次就还得从头猜。
            android.util.Log.w("MSIMECommunity", "Catalogue request failed", error);
            return new PageResponse(new Page(List.of(), false, CommunityRequest.message(null, 0)), 0);
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    /**
     * Report one entry to the moderators (POST /v1/community/reports).
     *
     * <p>Needs a signed-in session, and the device's anonymous account counts: the Google account when there is one, otherwise the keyboard's anonymous identity.
     *
     * @return the failure to show, or an empty string once the report was taken
     */
    public String report(Item item, String reason, String detail) {
        String text = detail == null ? "" : detail.trim();
        if (item == null || !CommunityRequest.validReport(reason, text)) {
            return CommunityRequest.message("invalid_report_reason", 400);
        }
        BackendAccount account = new BackendAccount(context);
        String token = account.accessToken();
        boolean anonymous = false;
        if (token.isEmpty()) {
            try {
                token = new BackendAnonymousAccount(context).accessToken();
                anonymous = true;
            } catch (Exception | LinkageError error) {
                android.util.Log.i("MSIMECommunity", "No identity to report with", error);
                return CommunityRequest.message(null, error instanceof BackendAnonymousAccount.RateLimited ? 429 : 0);
            }
        }
        HttpsURLConnection connection = null;
        try {
            JSONObject body = new JSONObject()
                .put("kind", CommunityRequest.reportKind(item.kind()))
                .put("item_id", item.id())
                .put("reason", reason);
            if (!text.isEmpty()) body.put("detail", text);
            connection = (HttpsURLConnection) new URL(ORIGIN + CommunityRequest.REPORT_PATH).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(TIMEOUT_MILLIS);
            connection.setReadTimeout(TIMEOUT_MILLIS);
            connection.setDoOutput(true);
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("Content-Type", "application/json");
            connection.setRequestProperty("User-Agent", "MSIME/Android");
            connection.setRequestProperty("Authorization", "Bearer " + token);
            try (java.io.OutputStream output = connection.getOutputStream()) {
                output.write(body.toString().getBytes(StandardCharsets.UTF_8));
            }
            int status = connection.getResponseCode();
            if (status == 200 || status == 201) return "";
            if (status == 401) {
                String fresh = anonymous
                    ? new BackendAnonymousAccount(context).accessToken(token)
                    : account.currentAccessToken(token);
                if (!fresh.isEmpty()) return reportWithToken(item, reason, text, fresh);
            }
            return CommunityRequest.message(errorCode(connection.getErrorStream()), status);
        } catch (Exception | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Report failed", error);
            return CommunityRequest.message(null, 0);
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private String reportWithToken(Item item, String reason, String text, String token) {
        HttpsURLConnection connection = null;
        try {
            JSONObject body = new JSONObject()
                .put("kind", CommunityRequest.reportKind(item.kind()))
                .put("item_id", item.id())
                .put("reason", reason);
            if (!text.isEmpty()) body.put("detail", text);
            connection = (HttpsURLConnection) new URL(ORIGIN + CommunityRequest.REPORT_PATH).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(TIMEOUT_MILLIS);
            connection.setReadTimeout(TIMEOUT_MILLIS);
            connection.setDoOutput(true);
            connection.setRequestProperty("Accept", "application/json");
            connection.setRequestProperty("Content-Type", "application/json");
            connection.setRequestProperty("User-Agent", "MSIME/Android");
            connection.setRequestProperty("Authorization", "Bearer " + token);
            try (java.io.OutputStream output = connection.getOutputStream()) {
                output.write(body.toString().getBytes(StandardCharsets.UTF_8));
            }
            int status = connection.getResponseCode();
            if (status == 200 || status == 201) return "";
            return CommunityRequest.message(errorCode(connection.getErrorStream()), status);
        } catch (Exception | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Report retry failed", error);
            return CommunityRequest.message(null, 0);
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    /**
     * 读目录用的令牌：登录了水杉账号就用账号的，这样服务端才能把作者自己发布的皮肤标成 `owned`，作者才看得到修改分类的入口；没登录用键盘的匿名身份；两者都拿不到就不带令牌。
     */
    private ListingToken listingToken() {
        try {
            String account = new BackendAccount(context).accessToken();
            if (!account.isEmpty()) return new ListingToken(account, false);
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Account session unavailable; trying anonymous",
                error);
        }
        try {
            return new ListingToken(new BackendAnonymousAccount(context).accessToken(), true);
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Anonymous identity unavailable; listing anyway",
                error);
            return new ListingToken(null, true);
        }
    }

    /**
     * 作者修改自己一款皮肤的分类。Never throws: a failure is an update that says why.
     *
     * <p>要求登录水杉账号：匿名身份发布不了皮肤，也就不可能是作者。服务端回显的分类和请求的不一致，说明修改没有生效，按失败处理。
     */
    public Update setCategory(Item item, CommunityRequest.Category category) {
        if (item == null || item.kind() != CommunityRequest.Kind.SKIN || category == null) {
            return new Update(null, "这类作品没有分类。");
        }
        BackendAccount account = new BackendAccount(context);
        String token = account.accessToken();
        if (token.isEmpty()) return new Update(null, "请先登录水杉账号，再修改分类。");
        for (int attempt = 0; ; attempt++) {
            HttpsURLConnection connection = null;
            try {
                connection = (HttpsURLConnection) new URL(
                    ORIGIN + CommunityRequest.skinPath(item.id())).openConnection();
                connection.setInstanceFollowRedirects(false);
                connection.setRequestMethod("PATCH");
                connection.setConnectTimeout(TIMEOUT_MILLIS);
                connection.setReadTimeout(TIMEOUT_MILLIS);
                connection.setDoOutput(true);
                connection.setRequestProperty("Accept", "application/json");
                connection.setRequestProperty("Content-Type", "application/json");
                connection.setRequestProperty("User-Agent", "MSIME/Android");
                connection.setRequestProperty("Authorization", "Bearer " + token);
                byte[] body = CommunityRequest.categoryBody(category).getBytes(StandardCharsets.UTF_8);
                connection.setFixedLengthStreamingMode(body.length);
                try (java.io.OutputStream output = connection.getOutputStream()) {
                    output.write(body);
                }
                int status = connection.getResponseCode();
                if (shouldRetryCategory(status, token, attempt)) {
                    String fresh = account.currentAccessToken(token);
                    if (!fresh.isEmpty() && !fresh.equals(token)) {
                        token = fresh;
                        continue;
                    }
                }
                if (status != 200) {
                    String code = errorCode(connection.getErrorStream());
                    return new Update(null, CommunityRequest.message(code, status));
                }
                Item updated;
                try (InputStream input = connection.getInputStream()) {
                    updated = item(CommunityRequest.Kind.SKIN, new JSONObject(new String(
                        readBounded(input, MAX_RESPONSE_BYTES), StandardCharsets.UTF_8)));
                }
                if (updated == null || !updated.id().equalsIgnoreCase(item.id())
                        || updated.category() != category) {
                    return new Update(null, CommunityRequest.message(null, 500));
                }
                return new Update(updated, "");
            } catch (Exception | LinkageError error) {
                android.util.Log.w("MSIMECommunity", "Category update failed", error);
                return new Update(null, CommunityRequest.message(null, 0));
            } finally {
                if (connection != null) connection.disconnect();
            }
        }
    }

    /** A category mutation may refresh its rejected account token exactly once. */
    static boolean shouldRetryCategory(int status, String token, int attempt) {
        return status == 401 && token != null && !token.isEmpty() && attempt == 0;
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
            Item item = value == null ? null : item(kind, value);
            if (item == null) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            if (!ids.add(idKey(item.id()))) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            items.add(item);
        }
        if (invalidPage(values.length(), items.size(), hasMore)) {
            return new Page(List.of(), false, CommunityRequest.message(null, 500));
        }
        return new Page(List.copyOf(items), hasMore, "");
    }

    static String idKey(String value) {
        return value.toLowerCase(Locale.ROOT);
    }

    /** 一个条目，读不出或不合规时为 null。 */
    private static Item item(CommunityRequest.Kind kind, JSONObject value) {
        boolean skin = kind == CommunityRequest.Kind.SKIN;
        String id = strictString(value.opt("id"));
        String name = strictString(value.opt("name"));
        if (id == null || name == null) return null;
        name = name.trim();
        JSONObject payload = skin ? value.optJSONObject("design") : value.optJSONObject("content");
        Long saves = count(value, "saves", skin ? "downloads" : null);
        Long ratings = count(value, "rating_count", null);
        Long downloads = count(value, "downloads", null);
        Double average = decimal(value, "rating_average");
        if (saves == null || ratings == null || downloads == null || average == null) return null;
        if (kind == CommunityRequest.Kind.PHRASE && !validPhrases(payload)) return null;
        CommunityRequest.Category category = null;
        if (skin) {
            Object raw = value.opt("category");
            category = CommunityRequest.Category.parse(raw == JSONObject.NULL ? null : raw);
            if (category == null) return null;
        }
        String description = value.has("description") ? strictString(value.opt("description")) : "";
        String author = value.has("author") ? strictString(value.opt("author")) : "";
        Boolean owned = value.has("owned") ? strictBoolean(value.opt("owned")) : Boolean.FALSE;
        if (description == null || author == null || owned == null) return null;
        Item item = new Item(id, kind, name, description.trim(), author.trim(), saves, ratings,
            average, payload, category, owned, downloads, value);
        return validItem(item, kind) ? item : null;
    }

    /** 短语包的内容 `{phrases:[{text,group}]}`：1–200 条，每条文字与分组都合规。 */
    private static boolean validPhrases(JSONObject content) {
        JSONArray phrases = content == null ? null : content.optJSONArray("phrases");
        if (phrases == null || !CommunityRequest.validPhraseCount(phrases.length())) return false;
        for (int index = 0; index < phrases.length(); index++) {
            JSONObject phrase = phrases.optJSONObject(index);
            if (phrase == null) return false;
            String text = strictString(phrase.opt("text"));
            Object rawGroup = phrase.opt("group");
            String group = rawGroup == null || rawGroup == JSONObject.NULL ? "" : strictString(rawGroup);
            if (!CommunityRequest.validPhraseText(text) || !CommunityRequest.validPhraseGroup(group)) return false;
        }
        return true;
    }

    /** org.json's optString/optBoolean coerce numbers and booleans; community responses are a typed contract. */
    static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }

    static Boolean strictBoolean(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
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
                || item.downloads() < 0 || item.downloads() > MAX_JAVASCRIPT_INTEGER
                || item.ratingCount() < 0 || item.ratingCount() > MAX_JAVASCRIPT_INTEGER
                || item.ratingAverage() < 0 || item.ratingAverage() > 5
                || Double.isNaN(item.ratingAverage()) || Double.isInfinite(item.ratingAverage())
                || !validCategory(kind, item.category())) {
            return false;
        }
        return item.ratingCount() != 0 || item.ratingAverage() == 0;
    }

    /** 皮肤一定有分类（缺失已在解析时读作 other），词库、回复和短语一定没有。 */
    static boolean validCategory(CommunityRequest.Kind kind, CommunityRequest.Category category) {
        return (kind == CommunityRequest.Kind.SKIN) == (category != null);
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
        return countNumber(raw);
    }

    static Long countNumber(Object raw) {
        if (!(raw instanceof Number number)) return null;
        if (!(raw instanceof Integer) && !(raw instanceof Long)) return null;
        long integer = number.longValue();
        return integer >= 0 && integer <= MAX_JAVASCRIPT_INTEGER ? integer : null;
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
