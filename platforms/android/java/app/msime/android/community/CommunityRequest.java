package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.util.List;

/**
 * 社区目录的请求路径与失败文案。
 *
 * <p>Kept apart from the transport so both can be read without a network: what a query looks like
 * and what a failure says are the two things that actually go wrong here, and neither needs a
 * socket to check.
 */
public final class CommunityRequest {
    /** 目录里的三类内容。皮肤自成一个端点，词库和回复共用资源端点。 */
    public enum Kind {
        SKIN("skin", "皮肤", "搜索皮肤设计"),
        DICTIONARY("dictionary", "词库", "搜索词包"),
        REPLY("reply", "回复", "搜索回复模板");

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

    /** One page of results. */
    public static final int PAGE_SIZE = 20;

    private CommunityRequest() {}

    public static List<Kind> kinds() { return List.of(Kind.values()); }

    /** The catalogue path for one kind, scope and search term. */
    public static String path(Kind kind, String scope, String search, int offset) {
        String bounded = search == null ? "" : search.trim();
        int page = Math.max(0, offset);
        if (kind == Kind.SKIN) {
            return "/v1/community/skins?offset=" + page + "&q=" + encode(bounded);
        }
        return "/v1/community/resources?kind=" + kind.id()
            + "&scope=" + encode(scope == null ? "" : scope)
            + "&q=" + encode(bounded) + "&offset=" + page;
    }

    /**
     * Percent-encode one query value.
     *
     * <p>Not {@code URLEncoder}: that is form encoding, where a space becomes `+` and a literal `+`
     * survives unescaped. In a query the server reads as percent-encoded, a search for `C++` then
     * arrives as two spaces.
     */
    public static String encode(String value) {
        if (value == null || value.isEmpty()) return "";
        StringBuilder result = new StringBuilder();
        for (byte raw : value.getBytes(StandardCharsets.UTF_8)) {
            int octet = raw & 0xFF;
            if (octet >= 'a' && octet <= 'z' || octet >= 'A' && octet <= 'Z'
                    || octet >= '0' && octet <= '9'
                    || octet == '-' || octet == '_' || octet == '.' || octet == '~') {
                result.append((char) octet);
            } else {
                result.append('%').append(Character.toUpperCase(
                    Character.forDigit(octet >>> 4, 16))).append(Character.toUpperCase(
                    Character.forDigit(octet & 0x0F, 16)));
            }
        }
        return result.toString();
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
