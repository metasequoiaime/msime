package app.msime.android;

import android.content.Context;
import java.io.InputStream;
import java.net.URL;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
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
    private static final UUID NIL_UUID = new UUID(0L, 0L);

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

    /** 设置页发现区的一次有界目录结果；失败时 `items` 为 null，`failure` 可直接展示。 */
    public record Discovery(List<Item> items, String failure) {
        public boolean failed() { return items == null; }
    }

    /** 把目录页转换成发现区使用的有界列表或失败状态。 */
    public static Discovery discovery(Page page, int limit) {
        if (page == null) return new Discovery(null, "暂时连不上社区，稍后再试。");
        if (page.failed()) return new Discovery(null, page.failure());
        return new Discovery(CommunityRequest.limitedCopy(page.items(), limit), null);
    }

    private final CloudApi cloud;
    private final CloudApi.Tokens accountTokens;
    private final CloudApi.Tokens anonymousTokens;
    private final CloudApi.Transport listingTransport;
    private record PageResponse(Page page, int status) {}
    private record ListingToken(String value, boolean anonymous, String sessionId) {}

    public CommunityCatalog(Context context) {
        this(context, new CloudApi(context));
    }

    CommunityCatalog(Context context, CloudApi cloud) {
        this(context, cloud, CloudApi.accountTokens(context.getApplicationContext()),
            rejected -> new BackendAnonymousAccount(context.getApplicationContext()).accessToken(rejected),
            CommunityCatalog::httpListingExchange);
    }

    CommunityCatalog(Context context, CloudApi cloud, CloudApi.Tokens accountTokens,
            CloudApi.Tokens anonymousTokens, CloudApi.Transport listingTransport) {
        this.cloud = cloud;
        this.accountTokens = accountTokens;
        this.anonymousTokens = anonymousTokens;
        this.listingTransport = listingTransport;
    }

    /** Open a catalogue request with the shared transport defaults. */
    private static HttpsURLConnection open(URL url, String method)
            throws java.io.IOException {
        HttpsURLConnection connection = (HttpsURLConnection) url.openConnection();
        HttpConnectionPolicy.rejectRedirects(connection);
        connection.setRequestMethod(method);
        HttpConnectionPolicy.setTimeouts(connection, TIMEOUT_MILLIS, TIMEOUT_MILLIS);
        connection.setRequestProperty("Accept", "application/json");
        connection.setRequestProperty("User-Agent", "MSIME/Android");
        return connection;
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
            Page stale = listingSessionFailure(selected);
            if (stale != null) return stale;
            PageResponse response = requestPage(kind, search, offset, category, token);
            stale = listingSessionFailure(selected);
            if (stale != null) return stale;
            if (!shouldRetryListing(response.status(), token, attempt)) return response.page();
            try {
                String fresh;
                if (selected.anonymous()) {
                    fresh = anonymousTokens.token(token);
                } else {
                    CloudApi.TokenSnapshot refreshed = accountTokens.snapshot(token);
                    if (!selected.sessionId().equals(refreshed.sessionId()))
                        return listingFailure("session_changed", 409);
                    fresh = refreshed.token();
                }
                if (fresh == null || fresh.isEmpty() || fresh.equals(token)) return response.page();
                token = fresh;
            } catch (java.util.concurrent.CancellationException changed) {
                return listingFailure("session_changed", 409);
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

    private Page listingSessionFailure(ListingToken selected) {
        if (selected.anonymous()) return null;
        if (selected.sessionId() == null || selected.sessionId().isEmpty())
            return listingFailure("session_unavailable", 0);
        try {
            CloudApi.TokenSnapshot current = accountTokens.snapshot(null);
            return selected.sessionId().equals(current.sessionId())
                ? null : listingFailure("session_changed", 409);
        } catch (java.util.concurrent.CancellationException changed) {
            return listingFailure("session_changed", 409);
        } catch (Exception | LinkageError unavailable) {
            android.util.Log.i("MSIMECommunity", "Account session check for catalogue failed", unavailable);
            return listingFailure("session_unavailable", 0);
        }
    }

    private static Page listingFailure(String code, int status) {
        return new Page(List.of(), false, CommunityRequest.message(code, status));
    }

    private PageResponse requestPage(CommunityRequest.Kind kind, String search, int offset,
            CommunityRequest.Category category, String token) {
        try {
            Map<String, String> headers = new LinkedHashMap<>();
            headers.put("Accept", "application/json");
            headers.put("User-Agent", "MSIME/Android");
            if (token != null) headers.put("Authorization", "Bearer " + token);
            CloudApi.Exchange response = listingTransport.exchange("GET",
                CommunityRequest.path(kind, "", search, offset, category), headers, null);
            int status = response.status();
            if (status != 200) {
                String code = CloudApi.failure(response).code;
                return new PageResponse(
                    new Page(List.of(), false, CommunityRequest.message(code, status)), status);
            }
            byte[] body = response.body();
            if (body == null || body.length > maximumResponseBytes(kind))
                throw new java.io.IOException("catalogue response too large");
            return new PageResponse(parse(kind, new JSONObject(TextPolicy.utf8(body))), 200);
        } catch (Exception | LinkageError error) {
            // 说出是哪一步断的。界面上仍然只有那一句，但把原因扔掉，下一次就还得从头猜。
            android.util.Log.w("MSIMECommunity", "Catalogue request failed", error);
            return new PageResponse(new Page(List.of(), false, CommunityRequest.message(null, 0)), 0);
        }
    }

    /** Catalogue pages may be larger than the shared cloud transport's 4 MiB default. */
    private static CloudApi.Exchange httpListingExchange(String method, String path,
            Map<String, String> headers, byte[] request) throws java.io.IOException {
        HttpsURLConnection connection = open(new URL(ORIGIN + path), method);
        try {
            for (Map.Entry<String, String> header : headers.entrySet())
                connection.setRequestProperty(header.getKey(), header.getValue());
            int status = connection.getResponseCode();
            InputStream stream = status == 200 ? connection.getInputStream() : connection.getErrorStream();
            byte[] body = new byte[0];
            if (stream != null) {
                int limit = status == 200 && path.startsWith("/v1/community/resources?")
                    ? MAX_RESOURCE_RESPONSE_BYTES : MAX_RESPONSE_BYTES;
                try (InputStream input = stream) {
                    if (status == 200) {
                        body = HttpBodyPolicy.readRequired(input, limit);
                    } else {
                        // A malformed or oversized error body must not hide its HTTP status.
                        try { body = HttpBodyPolicy.readRequired(input, limit); }
                        catch (java.io.IOException unreadable) { body = new byte[0]; }
                    }
                }
            }
            return new CloudApi.Exchange(status, connection.getContentType(),
                connection.getHeaderField("Retry-After"), body);
        } finally {
            connection.disconnect();
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
        String text = TextPolicy.trimmed(detail);
        if (!validReportItem(item) || !CommunityRequest.validReport(reason, text)) {
            return CommunityRequest.message("invalid_report_reason", 400);
        }
        try {
            JSONObject body = new JSONObject()
                .put("kind", CommunityRequest.reportKind(item.kind()))
                .put("item_id", item.id())
                .put("reason", reason);
            if (!text.isEmpty()) body.put("detail", text);
            CloudApi.Response reply = cloud.send("POST", CommunityRequest.REPORT_PATH,
                CloudApi.Body.json(body), CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
            if (reply.status() != 200 && reply.status() != 201)
                return CommunityRequest.message(null, reply.status());
            if (reply.body() == null || reply.body().length > 16 * 1024)
                return CommunityRequest.message(null, 502);
            JSONObject response = reply.json();
            return JsonPolicy.strictTrue(response.opt("reported"))
                ? "" : CommunityRequest.message(null, 502);
        } catch (CloudApi.Failure failure) {
            return CommunityRequest.message(failure.code, failure.status);
        } catch (Exception | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Report failed", error);
            return CommunityRequest.message(null, 0);
        }
    }

    /**
     * 读目录用的令牌：登录了水杉账号就用账号的，这样服务端才能把作者自己发布的皮肤标成 `owned`，作者才看得到修改分类的入口；没登录用键盘的匿名身份；两者都拿不到就不带令牌。
     */
    private ListingToken listingToken() {
        try {
            CloudApi.TokenSnapshot account = accountTokens.snapshot(null);
            if (account.token() != null && !account.token().isEmpty())
                return new ListingToken(account.token(), false, account.sessionId());
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Account session unavailable; trying anonymous",
                error);
        }
        try {
            return new ListingToken(anonymousTokens.token(null), true, null);
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Anonymous identity unavailable; listing anyway",
                error);
            return new ListingToken(null, true, null);
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
        try {
            CloudApi.Response response = cloud.send("PATCH", CommunityRequest.skinPath(item.id()),
                CloudApi.Body.json(new JSONObject(CommunityRequest.categoryBody(category))), CloudApi.Auth.ACCOUNT);
            if (response.status() != 200) return new Update(null, CommunityRequest.message(null, 500));
            Item updated = item(CommunityRequest.Kind.SKIN, response.json());
            if (updated == null || !updated.id().equalsIgnoreCase(item.id())
                    || updated.category() != category) {
                return new Update(null, CommunityRequest.message(null, 500));
            }
            return new Update(updated, "");
        } catch (CloudApi.Failure failure) {
            if ("signed_out".equals(failure.code))
                return new Update(null, "请先登录水杉账号，再修改分类。");
            return new Update(null, CommunityRequest.message(failure.code, failure.status));
        } catch (Exception | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Category update failed", error);
            return new Update(null, CommunityRequest.message(null, 0));
        }
    }

    private static Page parse(CommunityRequest.Kind kind, JSONObject root) {
        JSONArray values = root.optJSONArray(
            kind == CommunityRequest.Kind.SKIN ? "skins" : "items");
        if (values == null) return new Page(List.of(), false, CommunityRequest.message(null, 500));
        int rawLength = values.length();
        if (rawLength > CommunityRequest.PAGE_SIZE) {
            return new Page(List.of(), false, CommunityRequest.message(null, 500));
        }
        boolean hasMore = JsonPolicy.strictTrue(root.opt("has_more"));
        List<Item> items = new ArrayList<>(rawLength);
        Set<String> ids = new HashSet<>(rawLength);
        for (int index = 0; index < rawLength; index++) {
            JSONObject value = values.optJSONObject(index);
            Item item = value == null ? null : item(kind, value);
            if (item == null) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            if (!ids.add(TextPolicy.lowercase(item.id()))) {
                return new Page(List.of(), false, CommunityRequest.message(null, 500));
            }
            items.add(item);
        }
        if (invalidPage(rawLength, items.size(), hasMore)) {
            return new Page(List.of(), false, CommunityRequest.message(null, 500));
        }
        return new Page(List.copyOf(items), hasMore, "");
    }

    /** 一个条目，读不出或不合规时为 null。 */
    private static Item item(CommunityRequest.Kind kind, JSONObject value) {
        boolean skin = kind == CommunityRequest.Kind.SKIN;
        String id = JsonPolicy.strictString(value.opt("id"));
        String name = JsonPolicy.strictString(value.opt("name"));
        if (id == null || name == null) return null;
        name = TextPolicy.trimmed(name);
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
        String description = value.has("description") ? JsonPolicy.strictString(value.opt("description")) : "";
        String author = value.has("author") ? JsonPolicy.strictString(value.opt("author")) : "";
        Boolean owned = value.has("owned") ? JsonPolicy.strictBoolean(value.opt("owned")) : Boolean.FALSE;
        if (description == null || author == null || owned == null) return null;
        Item item = new Item(id, kind, name, TextPolicy.trimmed(description), TextPolicy.trimmed(author), saves, ratings,
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
            String text = JsonPolicy.strictString(phrase.opt("text"));
            Object rawGroup = phrase.opt("group");
            String group = rawGroup == null || rawGroup == JSONObject.NULL ? "" : JsonPolicy.strictString(rawGroup);
            if (!CommunityRequest.validPhraseText(text) || !CommunityRequest.validPhraseGroup(group)) return false;
        }
        return true;
    }

    /** Keep the report endpoint safe even when a caller bypasses catalogue parsing. */
    static boolean validReportItem(Item item) {
        return item != null && validUuid(item.id());
    }

    /** Keep the download counter path safe even when a caller bypasses catalogue parsing. */
    static boolean validDownloadItem(Item item) {
        return item != null && item.kind() == CommunityRequest.Kind.SKIN && validUuid(item.id());
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
            UUID parsed = UUID.fromString(value);
            return !NIL_UUID.equals(parsed) && parsed.toString().equalsIgnoreCase(value);
        } catch (IllegalArgumentException error) {
            return false;
        }
    }

    private static boolean validName(String value, int maximum) {
        return value != null && !value.isEmpty() && TextPolicy.trimmed(value).equals(value)
            && TextPolicy.withinCodePoints(value, maximum)
            && !CommunityTextPolicy.hasDisallowedControl(value, false);
    }

    private static boolean validDescription(String value) {
        return TextPolicy.withinCodePoints(value, MAX_DESCRIPTION_CHARACTERS)
            && !CommunityTextPolicy.hasDisallowedControl(value, true);
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

    static int maximumResponseBytes(CommunityRequest.Kind kind) {
        return kind == CommunityRequest.Kind.SKIN ? MAX_RESPONSE_BYTES : MAX_RESOURCE_RESPONSE_BYTES;
    }

    /**
     * Save a downloaded skin design into the library the keyboard's skin picker reads.
     *
     * @return the failure to show, or an empty string on success
     */
    public String install(java.nio.file.Path preferencesDirectory, Item item) {
        if (item == null || item.kind() != CommunityRequest.Kind.SKIN || item.payload() == null) {
            return "这类作品还不能从这里保存。";
        }
        try {
            return CustomSkinLibrary.add(preferencesDirectory, item.id(), item.name(),
                item.payload()) ? "" : "皮肤库已满，请先在键盘里删掉一些。";
        } catch (java.io.IOException | RuntimeException error) {
            return "保存失败，请稍后重试。";
        }
    }

    /**
     * 获取一款皮肤后在服务端记一次下载（`POST /v1/community/skins/{id}/download`），这就是卡片上的「使用次数」。
     *
     * <p>尽力而为：皮肤已经在本机皮肤库里，记不上只是计数少一次，不影响使用，所以失败只记日志。身份与举报相同：有水杉账号用账号令牌，否则用键盘的匿名身份；令牌被拒时换新令牌重试一次。
     *
     * @return 服务端是否记下了这次下载
     */
    public boolean recordDownload(Item item) {
        if (!validDownloadItem(item)) return false;
        try {
            CloudApi.Response response = cloud.send("POST", CommunityRequest.skinDownloadPath(item.id()),
                CloudApi.Body.json(new JSONObject()), CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
            return response.status() == 200;
        } catch (Exception | LinkageError error) {
            android.util.Log.i("MSIMECommunity", "Skin download was not counted", error);
            return false;
        }
    }
}
