package app.msime.android;

import android.content.Context;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 读社区目录：皮肤、词包、回复模板和短语包；以及举报、作者改分类和「获取」一款皮肤。
 *
 * <p>全部经 Rust 的 {@code msime_client_community}（#8201 迁移计划 5c，见 {@link RustAccount#community}）：列表是 keyboard_skin_list 和 resource_list，令牌、刷新和 401 后的重试都在 Rust 里。目录本身是公开的，没有登录时 Rust 不带令牌去读；登录了就带登录账号的令牌，服务端才会把作者自己发布的皮肤标成 `owned`。举报和下载皮肤是登录账号优先、否则用键盘的匿名账号，两种会话都没有时先注册匿名账号再试一次；改分类只认登录账号。
 *
 * <p>每个操作都在请求前后读一次登录的账号编号，期间登录、退出或换了号就丢掉结果、说「登录已切换」，与原来按会话绑定的做法相同。
 *
 * <p>每个调用都会阻塞（读登录状态要等 Rust 的会话锁），不能在主线程上调用。
 */
public final class CommunityCatalog {
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

    /** 设置页发现区的一次有界目录结果；失败时 `items` 为 null，`failure` 可直接展示。 */
    public record Discovery(List<Item> items, String failure) {
        public boolean failed() { return items == null; }
    }

    /** 「获取」一款皮肤的结果：成功时是 Rust 下载并规范化过的设计，失败时是可以原样展示的原因。 */
    public record Download(JSONObject design, String failure) {
        public boolean failed() { return design == null; }
    }

    /** 把目录页转换成发现区使用的有界列表或失败状态。 */
    public static Discovery discovery(Page page, int limit) {
        if (page == null) return new Discovery(null, "暂时连不上社区，稍后再试。");
        if (page.failed()) return new Discovery(null, page.failure());
        return new Discovery(CommunityRequest.limitedCopy(page.items(), limit), null);
    }

    /** 社区操作和登录状态从哪里来：产品里是 Rust（{@link RustAccount}），设备冒烟换成假的。 */
    interface Calls {
        /** 跑一个 {@code msime_client_community} 操作并返回 value；`anonymousFallback` 见 {@link RustAccount#community}。 */
        Object community(JSONObject action, boolean anonymousFallback) throws AccountStore.Failure;

        /** 现在登录的账号编号，没有登录时为空串。 */
        String userId() throws AccountStore.Failure;
    }

    private final Calls calls;

    public CommunityCatalog(Context context) {
        Context application = context.getApplicationContext();
        this.calls = new Calls() {
            @Override public Object community(JSONObject action, boolean anonymousFallback)
                    throws AccountStore.Failure {
                return RustAccount.community(application, action, null, anonymousFallback);
            }

            @Override public String userId() throws AccountStore.Failure {
                return RustAccount.userId(application);
            }
        };
    }

    CommunityCatalog(Calls calls) {
        this.calls = calls;
    }

    /** 一次社区操作，前后各读一次登录的账号编号；变了时抛 community_cancelled。 */
    private Object bound(JSONObject action, boolean anonymousFallback) throws AccountStore.Failure {
        String before = calls.userId();
        Object value = calls.community(action, anonymousFallback);
        if (!before.equals(calls.userId())) throw new AccountStore.Failure("community_cancelled", 0);
        return value;
    }

    /**
     * One page of the catalogue. Never throws: a failure is a page that says why.
     *
     * @param category 只列这一类皮肤；null 列出全部
     */
    public Page list(CommunityRequest.Kind kind, String search, int offset,
            CommunityRequest.Category category) {
        try {
            JSONObject action = kind == CommunityRequest.Kind.SKIN
                ? new JSONObject().put("operation", "keyboard_skin_list")
                    .put("offset", BoundsPolicy.nonNegative(offset)).put("search", TextPolicy.trimmed(search))
                : new JSONObject().put("operation", "resource_list").put("kind", kind.id()).put("scope", "")
                    .put("search", TextPolicy.trimmed(search)).put("offset", BoundsPolicy.nonNegative(offset));
            if (kind == CommunityRequest.Kind.SKIN && category != null) action.put("category", category.id());
            Object value = bound(action, false);
            if (!(value instanceof JSONObject root)) return failurePage(null);
            return parse(kind, root);
        } catch (AccountStore.Failure failure) {
            return failurePage(failure.code);
        } catch (JSONException | RuntimeException | LinkageError error) {
            // 说出是哪一步断的。界面上仍然只有那一句，但把原因扔掉，下一次就还得从头猜。
            android.util.Log.w("MSIMECommunity", "Catalogue request failed", error);
            return failurePage(null);
        }
    }

    private static Page failurePage(String code) {
        return new Page(List.of(), false, CommunityRequest.rustMessage(code));
    }

    /**
     * 向审核举报一个条目（Rust 的 report）。
     *
     * <p>要有会话，本机的匿名账号也算：有登录账号时用登录账号，否则用键盘的匿名身份。
     *
     * @return 要显示的失败文案，举报被受理时是空字符串
     */
    public String report(Item item, String reason, String detail) {
        String text = TextPolicy.trimmed(detail);
        if (!validReportItem(item) || !CommunityRequest.validReport(reason, text)) {
            return CommunityRequest.message("invalid_report_reason", 400);
        }
        try {
            JSONObject action = new JSONObject().put("operation", "report")
                .put("kind", CommunityRequest.reportKind(item.kind())).put("id", item.id()).put("reason", reason);
            if (!text.isEmpty()) action.put("detail", text);
            bound(action, true);
            return "";
        } catch (AccountStore.Failure failure) {
            return CommunityRequest.rustMessage(failure.code);
        } catch (JSONException | RuntimeException | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Report failed", error);
            return CommunityRequest.rustMessage(null);
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
            Object value = bound(new JSONObject().put("operation", "keyboard_skin_set_category")
                .put("id", item.id()).put("category", category.id()), false);
            Item updated = value instanceof JSONObject skin ? item(CommunityRequest.Kind.SKIN, skin) : null;
            if (updated == null || !updated.id().equalsIgnoreCase(item.id())
                    || updated.category() != category) {
                return new Update(null, CommunityRequest.message(null, 500));
            }
            return new Update(updated, "");
        } catch (AccountStore.Failure failure) {
            if ("community_unauthorized".equals(failure.code))
                return new Update(null, "请先登录水杉账号，再修改分类。");
            return new Update(null, CommunityRequest.rustMessage(failure.code));
        } catch (JSONException | RuntimeException | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Category update failed", error);
            return new Update(null, CommunityRequest.rustMessage(null));
        }
    }

    /**
     * 「获取」的第一步：经 Rust 的 keyboard_skin_download 下载这款皮肤的设计，服务端同时记一次下载（卡片上的「使用次数」）。登录账号优先、否则用键盘的匿名账号。断网时失败，不再像原来那样先存本机、再尽力记下载（决定 14）。Never throws.
     */
    public Download download(Item item) {
        if (!validDownloadItem(item)) return new Download(null, "这类作品还不能从这里保存。");
        try {
            Object value = bound(new JSONObject().put("operation", "keyboard_skin_download").put("id", item.id()), true);
            if (!(value instanceof JSONObject design)) return new Download(null, CommunityRequest.message(null, 500));
            return new Download(design, "");
        } catch (AccountStore.Failure failure) {
            return new Download(null, CommunityRequest.rustMessage(failure.code));
        } catch (JSONException | RuntimeException | LinkageError error) {
            android.util.Log.w("MSIMECommunity", "Skin download failed", error);
            return new Download(null, CommunityRequest.rustMessage(null));
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
        return JsonPolicy.nonNilUuid(value);
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
        Long integer = JsonPolicy.nonNegativeLong(raw);
        if (integer == null) return null;
        return integer >= 0 && integer <= MAX_JAVASCRIPT_INTEGER ? integer : null;
    }

    private static Double decimal(JSONObject value, String key) {
        Object raw = value.opt(key);
        if (raw == null || raw == JSONObject.NULL) return 0.0;
        double result = NumberPolicy.strictDouble(raw, Double.NaN);
        return Double.isFinite(result) ? result : null;
    }

    /**
     * 「获取」的第二步：把 {@link #download} 拿到的设计存进键盘皮肤面板读的那个库（Java 的 {@link CustomSkinLibrary}，键盘进程经它的锁文件读写同一份）。只存库、不换上，「使用」是另一个按钮。
     *
     * @return the failure to show, or an empty string on success
     */
    public static String install(java.nio.file.Path preferencesDirectory, Item item, JSONObject design) {
        if (item == null || item.kind() != CommunityRequest.Kind.SKIN || design == null) {
            return "这类作品还不能从这里保存。";
        }
        try {
            return CustomSkinLibrary.add(preferencesDirectory, item.id(), item.name(), design)
                ? "" : "皮肤库已满，请先在键盘里删掉一些。";
        } catch (java.io.IOException | RuntimeException error) {
            return "保存失败，请稍后重试。";
        }
    }
}
