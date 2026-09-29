import app.msime.android.CommunityRequest;
import app.msime.android.CommunityRequest.Kind;

public final class CommunityRequestSmoke {
    public static void main(String[] args) {
        check(CommunityRequest.kinds().size() == 3, "skins, dictionaries and replies");
        check("皮肤".equals(Kind.SKIN.title()) && !Kind.SKIN.searchHint().isEmpty(),
            "every kind is titled and says what its search covers");

        check("/v1/community/skins?offset=0&q=".equals(
            CommunityRequest.path(Kind.SKIN, "", "", 0)), "the skin catalogue has its own endpoint");
        check("/v1/community/resources?kind=dictionary&scope=mine&q=&offset=40".equals(
            CommunityRequest.path(Kind.DICTIONARY, "mine", "", 40)),
            "dictionaries and replies share the resource endpoint, separated by kind");
        check(CommunityRequest.path(Kind.REPLY, "", "", -5).endsWith("offset=0"),
            "a negative offset is the first page, not a server error");
        check(CommunityRequest.path(Kind.SKIN, "", "  海盐  ", 0).endsWith("q=%E6%B5%B7%E7%9B%90"),
            "a search term is trimmed and percent-encoded");

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
        System.out.println("Android community requests: paths, query encoding and failures passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
