package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 社区目录的分类、校验规则与失败文案。请求路径和查询的编码在 Rust 里（{@code msime_client_community}，#8201 迁移计划 5c），这里不再拼 URL。
 *
 * <p>与传输分开，不联网也能读：这里真正会出错的是失败时说什么，检查它不需要套接字。
 */
public final class CommunityRequest {
    /** 目录里的四类内容。皮肤自成一个端点，词库、回复模板和短语共用资源端点。只增不改：首页按名字引用这几个值。 */
    public enum Kind {
        SKIN("skin", "皮肤", "搜索皮肤设计"),
        DICTIONARY("dictionary", "词库", "搜索词包"),
        REPLY("reply", "回复模板", "搜索回复模板"),
        /** 不带编码的常用语包，装进本机常用语；服务端的 `/apply` 不接受它，合并在本机完成。 */
        PHRASE("phrase", "短语", "搜索短语");

        private final String id;
        private final String title;
        private final String searchHint;

        Kind(String id, String title, String searchHint) {
            this.id = id;
            this.title = title;
            this.searchHint = searchHint;
        }

        public String id() { return id; }

        public String title() { return title; }

        public String searchHint() { return searchHint; }
    }

    /**
     * 社区键盘皮肤的发布分类，顺序即筛选条上按钮的顺序。
     *
     * <p>分类只是发布元数据，不属于皮肤设计本身，也不进任何请求摘要。服务端将来新增的分类 id 一律读作 {@link #OTHER}，旧客户端不会因此读不出整页。
     */
    public enum Category {
        NATURE("nature", "自然"),
        GUOFENG("guofeng", "国风"),
        ACG("acg", "二次元"),
        CUTE("cute", "可爱"),
        FOOD("food", "美食"),
        TECH("tech", "科技夜色"),
        MINIMAL("minimal", "简约"),
        OTHER("other", "其他");

        private final String id;
        private final String label;

        Category(String id, String label) {
            this.id = id;
            this.label = label;
        }

        public String id() { return id; }

        public String label() { return label; }

        /**
         * 读条目里的 `category` 字段。
         *
         * <p>早于分类功能的服务端不返回这个字段（它只回给带了 `include=category` 的请求），缺失（调用方把 JSON `null` 也作为 Java null 传进来）读作 {@link #OTHER}；不认识的 id 同样读作 {@link #OTHER}。不是字符串的值是服务端故障，返回 null 让调用方按坏条目处理。参数是 `Object` 而不是 `JSONObject`，是为了让 JVM smoke 不碰 android.jar 里会抛异常的 org.json 桩。
         */
        public static Category parse(Object raw) {
            if (raw == null) return OTHER;
            if (!(raw instanceof String value)) return null;
            for (Category category : values()) {
                if (category.id.equals(value)) return category;
            }
            return OTHER;
        }
    }

    /** One page of results. */
    public static final int PAGE_SIZE = 20;

    /** The fixed report reasons, in dialog order, each the exact string the server accepts. The same list on every host. */
    public static final List<String> REPORT_REASONS =
        List.of("侵权/抄袭", "色情低俗", "违法违规", "垃圾广告", "恶意插件", "其他");
    /** The longest optional detail a report may carry, in characters (code points). */
    public static final int MAX_REPORT_DETAIL = 1000;

    private CommunityRequest() {}

    public static List<Kind> kinds() { return List.of(Kind.values()); }

    /** 社区页顶部的三个分段，顺序即展示顺序；回复模板放在「短语」分段里作为第二个小节「AI 回复模板」。 */
    public static List<Kind> segments() { return List.of(Kind.SKIN, Kind.DICTIONARY, Kind.PHRASE); }

    /** Copy at most {@code limit} catalogue entries for a bounded discovery section. */
    public static <T> List<T> limitedCopy(List<T> values, int limit) {
        if (values == null || values.isEmpty() || limit <= 0) return List.of();
        return new ArrayList<>(values.subList(0, BoundsPolicy.atMost(limit, values.size())));
    }

    /** 一个短语包最多 200 条。 */
    public static final int MAX_PHRASES = 200;
    /** 每条短语最多 2000 个 UTF-16 单元，与服务端的限制一致。 */
    public static final int MAX_PHRASE_UNITS = 2000;
    /** 分组名最多 32 个 Unicode 字符，与 client-services 的社区资源契约一致。 */
    public static final int MAX_PHRASE_GROUP_UNITS = 32;

    /** 短语包的条数是否在 1–200 之间。 */
    public static boolean validPhraseCount(int count) {
        return count >= 1 && count <= MAX_PHRASES;
    }

    /** 一条短语：1–2000 个 UTF-16 单元，除换行和制表符外不含控制字符（签名之类需要换行）。 */
    public static boolean validPhraseText(String text) {
        if (text == null || text.isEmpty() || text.length() > MAX_PHRASE_UNITS) return false;
        return !CommunityTextPolicy.hasDisallowedControl(text, true);
    }

    /** 分组名：可以为空，最多 32 个 Unicode 字符，不含任何控制字符。 */
    public static boolean validPhraseGroup(String group) {
        return TextPolicy.withinCodePoints(group, MAX_PHRASE_GROUP_UNITS)
            && !CommunityTextPolicy.hasDisallowedControl(group, false);
    }

    public static List<Category> categories() { return List.of(Category.values()); }

    /** The `kind` a report names this catalogue's items by. */
    public static String reportKind(Kind kind) {
        return switch (kind) {
            case SKIN -> "skins";
            case DICTIONARY -> "dictionaries";
            case REPLY -> "replies";
            case PHRASE -> "phrases";
        };
    }

    /** Whether a report can be sent as written: one of the fixed reasons, and a detail within the limit. */
    public static boolean validReport(String reason, String detail) {
        if (reason == null || !REPORT_REASONS.contains(reason)) return false;
        String text = detail == null ? "" : detail;
        if (!TextPolicy.withinCodePoints(text, MAX_REPORT_DETAIL)) return false;
        return !CommunityTextPolicy.hasDisallowedControl(text, true);
    }

    /** 皮肤卡上的使用次数：一万以下照写，一万起按「万」取一位小数（去掉 `.0`），如「15.8 万 次使用」。 */
    public static String usesLabel(long downloads) {
        long count = BoundsPolicy.nonNegative(downloads);
        if (count < 10_000) return count + " 次使用";
        long tenths = Math.round(count / 1_000.0);
        String value = tenths % 10 == 0 ? Long.toString(tenths / 10) : (tenths / 10) + "." + (tenths % 10);
        return value + " 万 次使用";
    }

    /**
     * 一个词库或短语包里的条数：词库数 `content.entries`，短语包数 `content.phrases`；回复模板和读不出的内容为 -1，界面上不写条数。
     */
    public static int entryCount(Kind kind, org.json.JSONObject content) {
        if (content == null) return -1;
        String key = switch (kind) {
            case DICTIONARY -> "entries";
            case PHRASE -> "phrases";
            default -> "";
        };
        if (key.isEmpty()) return -1;
        org.json.JSONArray values = content.optJSONArray(key);
        return values == null ? -1 : values.length();
    }

    /**
     * `updatedAt`（RFC 3339，来自条目原始 JSON 的 `updated_at`）是否在 `nowMillis` 之前七天以内；为空或读不出时为 false，界面上就不写「本周更新」。
     */
    public static boolean updatedThisWeek(String updatedAt, long nowMillis) {
        if (updatedAt == null || updatedAt.isEmpty()) return false;
        try {
            long updated = java.time.OffsetDateTime.parse(updatedAt).toInstant().toEpochMilli();
            long week = 7L * 24 * 60 * 60 * 1000;
            long earliest;
            try {
                earliest = Math.subtractExact(nowMillis, week);
            } catch (ArithmeticException beforeEpoch) {
                earliest = Long.MIN_VALUE;
            }
            return updated >= earliest && updated <= nowMillis;
        } catch (java.time.format.DateTimeParseException | ArithmeticException error) {
            return false;
        }
    }

    /**
     * 词库和短语行的副标题：「@作者 · 4,812 条 · 本周更新」；作者为空时不写作者，条数未知（负数）时不写条数。
     */
    public static String resourceSubtitle(String author, int entries, boolean updatedThisWeek) {
        List<String> parts = new java.util.ArrayList<>(3);
        if (author != null && !author.isEmpty()) parts.add("@" + author);
        if (entries >= 0) parts.add(NumberPolicy.groupedCount(entries));
        if (updatedThisWeek) parts.add("本周更新");
        return String.join(" · ", parts);
    }

    /**
     * Rust 社区入口（{@code msime_client_community}）的错误码说成一句话（#8201 迁移计划 5c）。措辞沿用 {@link #message} 按服务端错误码和状态码选的那几句；Rust 没有单独的断网码，连不上和服务端出错都是 community_unavailable，合成一句。注册匿名账号被限流时是 account_rate_limited，与社区的限流同一句。
     */
    public static String rustMessage(String code) {
        return switch (code == null ? "" : code) {
            case "community_unauthorized" -> message(null, 401);
            case "community_forbidden" -> message(null, 403);
            case "community_not_found" -> message("item_not_found", 404);
            case "community_rate_limited", "account_rate_limited" -> message(null, 429);
            case "community_cancelled", "account_cancelled" -> message("session_changed", 409);
            case "community_conflict" -> message(null, 409);
            case "community_blocked_content" -> message("blocked_content", 422);
            case "community_screening_unavailable" -> message("screening_unavailable", 503);
            case "community_account_banned" -> message("account_banned", 403);
            case "community_storage" -> "保存失败，请稍后重试。";
            case "community_invalid" -> "请求不符合要求，请刷新后重试。";
            default -> "连不上社区或社区暂时不可用，请稍后重试。";
        };
    }

    /**
     * What to tell the reader when a request fails.
     *
     * <p>The backend's own error code answers first; the status code is the fallback for a failure
     * it did not name. A bare "请求失败" for all of them would hide the two cases the reader can act
     * on -- not signed in, and rate limited.
     */
    public static String message(String code, int status) {
        String named = switch (code == null ? "" : code) {
            case "provider_disabled", "user_auth_disabled" -> "社区登录尚未启用，请稍后重试。";
            case "download_before_rating_or_own_skin" -> "下载使用后才能评分，且不能评价自己的作品。";
            case "skin_publish_limit" -> "最多发布 50 款皮肤，请先下架部分作品。";
            case "recent_login_required" -> "请退出并重新登录后，再注销账号。";
            case "invalid_skin_design", "invalid_skin_metadata" -> "皮肤内容或名称不符合发布要求。";
            // Content screening and moderation. A refused word is the text's problem, never a service that is down.
            case "blocked_content" -> "内容包含不允许发布的词语，请修改后再提交";
            case "screening_unavailable" -> "审核服务暂时不可用，请稍后重试";
            case "account_banned" -> "该账号已被封禁，暂时无法使用账号相关功能";
            case "session_changed" -> "登录已切换，请重试。";
            case "item_not_found" -> "作品不存在或已下架。";
            case "unsupported_kind" -> "这类作品需要在本机添加，请更新到最新版本后重试。";
            case "invalid_report_reason", "invalid_report_detail", "invalid_report_kind" ->
                "举报内容不符合要求，请重新选择原因。";
            default -> "";
        };
        if (!named.isEmpty()) return named;
        return switch (status) {
            case 401 -> "登录已过期，请重新登录。";
            case 403 -> "收藏后才能评分，且不能评价自己的作品。";
            case 404 -> "作品不存在或已下架。";
            case 400 -> "请检查名称、词条或提示词是否符合要求。";
            case 409 -> "作品已更新或达到发布上限，请刷新后重试。";
            case 429 -> "操作较频繁，请稍后重试。";
            case 0 -> "连不上社区，请检查网络后重试。";
            default -> "社区暂时不可用，请稍后重试。";
        };
    }
}
