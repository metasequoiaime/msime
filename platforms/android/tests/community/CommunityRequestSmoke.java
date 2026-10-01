import app.msime.android.CommunityRequest;
import app.msime.android.CommunityRequest.Category;
import app.msime.android.CommunityRequest.Kind;
import java.util.List;

public final class CommunityRequestSmoke {
    public static void main(String[] args) {
        check(CommunityRequest.kinds().size() == 3, "skins, dictionaries and replies");
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
        check("连不上社区，请检查网络后重试。".equals(CommunityRequest.message(null, 0)),
            "a request that never reached the server says so");
        check(!CommunityRequest.message("something new", 599).isEmpty(),
            "an unknown failure still says something");

        check("skins".equals(CommunityRequest.reportKind(Kind.SKIN))
            && "dictionaries".equals(CommunityRequest.reportKind(Kind.DICTIONARY))
            && "replies".equals(CommunityRequest.reportKind(Kind.REPLY)),
            "reports name each kind the way the server does");
        check(CommunityRequest.REPORT_REASONS.equals(java.util.List.of(
            "侵权/抄袭", "色情低俗", "违法违规", "垃圾广告", "恶意插件", "其他")),
            "the fixed report reasons, in order");
        check(CommunityRequest.validReport("其他", null) && CommunityRequest.validReport("其他", "😀".repeat(1000)),
            "a detail of up to 1000 characters is accepted");
        check(!CommunityRequest.validReport("其他", "a".repeat(1001)), "a longer detail is refused");
        check(!CommunityRequest.validReport("不喜欢", ""), "only the fixed reasons are sent");
        check("内容包含不允许发布的词语，请修改后再提交".equals(CommunityRequest.message("blocked_content", 422)),
            "a refused word asks for an edit");
        check("审核服务暂时不可用，请稍后重试".equals(CommunityRequest.message("screening_unavailable", 503)),
            "screening outage asks for a retry");
        check(CommunityRequest.message("account_banned", 403).contains("封禁"), "a ban is named");
        System.out.println("Android community requests: paths, categories, query encoding, reports and failures passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
