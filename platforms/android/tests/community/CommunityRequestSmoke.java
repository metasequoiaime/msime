import app.msime.android.CommunityRequest;
import app.msime.android.CommunityRequest.Category;
import app.msime.android.CommunityRequest.Kind;
import app.msime.android.NumberPolicy;
import java.util.List;

public final class CommunityRequestSmoke {
    public static void main(String[] args) {
        check(CommunityRequest.kinds().size() == 4, "skins, dictionaries, replies and phrases");
        check(List.of(Kind.SKIN, Kind.DICTIONARY, Kind.PHRASE).equals(CommunityRequest.segments()),
            "the tab shows skins, dictionaries and phrases; replies live inside phrases");
        check(List.of("a", "b").equals(CommunityRequest.limitedCopy(List.of("a", "b", "c"), 2)),
            "a bounded catalogue copy keeps order and truncates at the limit");
        check(CommunityRequest.limitedCopy(List.of("a"), 0).isEmpty(),
            "a non-positive catalogue limit returns an empty copy");
        check("phrase".equals(Kind.PHRASE.id()) && "短语".equals(Kind.PHRASE.title()), "phrase kind");
        check(CommunityRequest.path(Kind.PHRASE, "", "签名", 0).startsWith("/v1/community/resources?kind=phrase&"),
            "phrase packs share the resource endpoint");
        check(CommunityRequest.validPhraseCount(1) && CommunityRequest.validPhraseCount(200)
            && !CommunityRequest.validPhraseCount(0) && !CommunityRequest.validPhraseCount(201), "1 to 200 phrases");
        check(CommunityRequest.validPhraseText("此致\n敬礼") && CommunityRequest.validPhraseText("x".repeat(2000)),
            "a phrase may span lines up to 2000 units");
        check(!CommunityRequest.validPhraseText("") && !CommunityRequest.validPhraseText("x".repeat(2001))
            && !CommunityRequest.validPhraseText("a\u0000b"), "empty, long and control text is refused");
        check(!CommunityRequest.validPhraseText("bad\uD800text")
            && !CommunityRequest.validPhraseGroup("bad\uD800group"),
            "phrase text and groups reject malformed Unicode");
        check(CommunityRequest.validPhraseGroup("") && !CommunityRequest.validPhraseGroup("a\nb")
            && !CommunityRequest.validPhraseGroup("x".repeat(33)), "phrase groups are short single lines");
        check(CommunityRequest.validPhraseGroup("😀".repeat(32)),
            "phrase groups use the shared Unicode character bound");
        check("皮肤".equals(Kind.SKIN.title()) && !Kind.SKIN.searchHint().isEmpty(),
            "every kind is titled and says what its search covers");

        check("/v1/community/skins?offset=0&q=&include=category".equals(
            CommunityRequest.path(Kind.SKIN, "", "", 0)),
            "the skin catalogue has its own endpoint and always asks for categories");
        check("/v1/community/resources?kind=dictionary&scope=mine&q=&offset=40".equals(
            CommunityRequest.path(Kind.DICTIONARY, "mine", "", 40)),
            "dictionaries and replies share the resource endpoint, separated by kind");
        check(CommunityRequest.path(Kind.REPLY, "", "", -5).endsWith("offset=0"),
            "a negative offset is the first page, not a server error");
        check(CommunityRequest.path(Kind.SKIN, "", "  海盐  ", 0).contains("&q=%E6%B5%B7%E7%9B%90&"),
            "a search term is trimmed and percent-encoded");

        // 分类：固定的八个 id 和标签，顺序即筛选条的顺序。
        check(List.of("nature", "guofeng", "acg", "cute", "food", "tech", "minimal", "other")
            .equals(CommunityRequest.categories().stream().map(Category::id).toList()),
            "the eight category ids in the server's order");
        check(List.of("自然", "国风", "二次元", "可爱", "美食", "科技夜色", "简约", "其他")
            .equals(CommunityRequest.categories().stream().map(Category::label).toList()),
            "every category carries its label");
        check("/v1/community/skins?offset=20&q=%E6%B5%B7&category=guofeng&include=category".equals(
            CommunityRequest.path(Kind.SKIN, "", "海", 20, Category.GUOFENG)),
            "a category filter rides on the skin listing beside include=category");
        check(CommunityRequest.path(Kind.SKIN, "", "", 0, null)
            .equals(CommunityRequest.path(Kind.SKIN, "", "", 0)), "no category lists them all");
        check("/v1/community/resources?kind=reply&scope=&q=&offset=0".equals(
            CommunityRequest.path(Kind.REPLY, "", "", 0, Category.FOOD)),
            "resources have no categories, so neither the filter nor include=category is sent");
        String id = "0f8fad5b-d9cb-469f-a165-70867728950e";
        check(("/v1/community/skins/" + id + "?include=category").equals(CommunityRequest.skinPath(id)),
            "the owner's category edit addresses the skin and asks for its category back");
        check("{\"category\":\"tech\"}".equals(CommunityRequest.categoryBody(Category.TECH)),
            "the category edit body carries only the category id");
        for (Category category : Category.values()) {
            check(Category.parse(category.id()) == category, "every known id parses to itself");
        }
        check(Category.parse(null) == Category.OTHER,
            "a server older than categories omits the field; that reads as other");
        check(Category.parse("seasonal") == Category.OTHER,
            "a category id added later reads as other rather than failing the page");
        check(Category.parse("GUOFENG") == Category.OTHER, "ids are matched exactly");
        check(Category.parse(Integer.valueOf(3)) == null && Category.parse(Boolean.TRUE) == null,
            "a category that is not a string is a malformed item");

        // Form encoding would send this as two spaces: `+` is a literal here, not a space.
        check("C%2B%2B".equals(CommunityRequest.encode("C++")), "a plus stays a plus");
        check("a%20b".equals(CommunityRequest.encode("a b")), "a space is %20, not +");
        check("-_.~".equals(CommunityRequest.encode("-_.~")), "unreserved characters pass through");
        check(CommunityRequest.encode("").isEmpty() && CommunityRequest.encode(null).isEmpty(),
            "nothing to encode encodes to nothing");

        check("最多发布 50 款皮肤，请先下架部分作品。".equals(
            CommunityRequest.message("skin_publish_limit", 409)),
            "a named code answers before the status");
        check("登录已过期，请重新登录。".equals(CommunityRequest.message("", 401)),
            "an unnamed 401 falls back to the status");
        check("登录已切换，请重试。".equals(CommunityRequest.message("session_changed", 409)),
            "a replaced login asks for retry under the current account");
        check("连不上社区，请检查网络后重试。".equals(CommunityRequest.message(null, 0)),
            "a request that never reached the server says so");
        check(!CommunityRequest.message("something new", 599).isEmpty(),
            "an unknown failure still says something");

        check("skins".equals(CommunityRequest.reportKind(Kind.SKIN))
            && "dictionaries".equals(CommunityRequest.reportKind(Kind.DICTIONARY))
            && "replies".equals(CommunityRequest.reportKind(Kind.REPLY))
            && "phrases".equals(CommunityRequest.reportKind(Kind.PHRASE)),
            "reports name each kind the way the server does");
        check(CommunityRequest.REPORT_REASONS.equals(java.util.List.of(
            "侵权/抄袭", "色情低俗", "违法违规", "垃圾广告", "恶意插件", "其他")),
            "the fixed report reasons, in order");
        check(CommunityRequest.validReport("其他", null) && CommunityRequest.validReport("其他", "😀".repeat(1000)),
            "a detail of up to 1000 characters is accepted");
        check(CommunityRequest.validReport("其他", "换行\n制表\t"),
            "report details may contain ordinary line breaks and tabs");
        check(!CommunityRequest.validReport("其他", "bad\u0000detail")
            && !CommunityRequest.validReport("其他", "bad\u007fdetail"),
            "report details reject control characters the service cannot store");
        check(!CommunityRequest.validReport("其他", "bad\uD800detail"),
            "report details reject malformed Unicode");
        check(!CommunityRequest.validReport("其他", "a".repeat(1001)), "a longer detail is refused");
        check(!CommunityRequest.validReport("不喜欢", ""), "only the fixed reasons are sent");
        check("内容包含不允许发布的词语，请修改后再提交".equals(CommunityRequest.message("blocked_content", 422)),
            "a refused word asks for an edit");
        check("审核服务暂时不可用，请稍后重试".equals(CommunityRequest.message("screening_unavailable", 503)),
            "screening outage asks for a retry");
        check(CommunityRequest.message("account_banned", 403).contains("封禁"), "a ban is named");
        // 卡片上的计数与副标题。
        String skinId = "10000000-0000-4000-8000-000000000001";
        check(("/v1/community/skins/" + skinId + "/download").equals(CommunityRequest.skinDownloadPath(skinId)),
            "getting a skin counts a download on the skin's own endpoint");
        check("0 次使用".equals(CommunityRequest.usesLabel(0)) && "9999 次使用".equals(CommunityRequest.usesLabel(9_999))
            && "0 次使用".equals(CommunityRequest.usesLabel(-3)), "small use counts are written out");
        check("15.8 万 次使用".equals(CommunityRequest.usesLabel(158_000))
            && "1 万 次使用".equals(CommunityRequest.usesLabel(10_000))
            && "2.9 万 次使用".equals(CommunityRequest.usesLabel(29_049)), "large use counts are in 万 with one decimal");
        check("4,812 条".equals(NumberPolicy.groupedCount(4_812)), "entry counts are grouped by thousands");
        check("@水杉词库组 · 4,812 条 · 本周更新".equals(CommunityRequest.resourceSubtitle("水杉词库组", 4_812, true))
            && "18 条".equals(CommunityRequest.resourceSubtitle("", 18, false))
            && "@寻章".equals(CommunityRequest.resourceSubtitle("寻章", -1, false)),
            "a resource row says author, entries and a recent update, leaving out what it does not know");
        long now = java.time.OffsetDateTime.parse("2026-10-05T12:00:00Z").toInstant().toEpochMilli();
        check(CommunityRequest.updatedThisWeek("2026-10-01T08:00:00+08:00", now)
            && !CommunityRequest.updatedThisWeek("2026-09-20T00:00:00Z", now)
            && !CommunityRequest.updatedThisWeek("2026-10-06T00:00:00Z", now)
            && !CommunityRequest.updatedThisWeek("yesterday", now)
            && !CommunityRequest.updatedThisWeek(null, now), "only an update within the last seven days counts");
        check(!CommunityRequest.updatedThisWeek("+1000000000-01-01T00:00:00Z", now)
            && !CommunityRequest.updatedThisWeek("-1000000000-01-01T00:00:00Z", now),
            "overflowing update timestamps are ignored");
        check(!CommunityRequest.updatedThisWeek("-292275055-05-16T16:47:04.192Z", now),
            "old in-range timestamps do not pass through subtraction overflow");
        System.out.println("Android community requests: paths, categories, query encoding, reports and failures passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
