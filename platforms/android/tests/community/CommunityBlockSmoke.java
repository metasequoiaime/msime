import app.msime.android.CommunityRequest;
import app.msime.android.CommunityRequest.BlockedAuthor;
import app.msime.android.CommunityRequest.Kind;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * 屏蔽作者的文案、作品类型、Rust 答的屏蔽记录与名单的读法，以及失败码说成哪句话。请求形状和账号绑定要真的 org.json，由设备套件 `BackendAccountSessionDeviceSmoke` 的假 Rust 覆盖；这里只碰不依赖 org.json 的那一半。
 */
public final class CommunityBlockSmoke {
    private static final String BLOCK = "20000000-0000-4000-8000-000000000002";
    private static final String OTHER = "30000000-0000-4000-8000-000000000003";

    public static void main(String[] args) {
        // 三个平台同一套文案。
        check("屏蔽作者「合成作者」？".equals(CommunityRequest.blockTitle("合成作者")), "the confirmation title names the author");
        check(("屏蔽后，社区里不再显示这位作者发布的皮肤、插件、词库和模板。可以在「我的 › 隐私 › 已屏蔽的作者」里取消屏蔽。")
            .equals(CommunityRequest.BLOCK_CONFIRM_MESSAGE), "the confirmation message matches iOS and HarmonyOS");
        check("已屏蔽「合成作者」".equals(CommunityRequest.blockedNotice("合成作者")), "the success notice names the author");
        check("没有屏蔽任何作者。在社区作品详情里点「屏蔽作者」，就不会再看到这位作者的作品。"
            .equals(CommunityRequest.BLOCKED_AUTHORS_EMPTY), "the empty list explains where blocking happens");
        check("登录水杉账号后才能屏蔽作者。".equals(CommunityRequest.BLOCK_SIGN_IN), "the sign-in prompt");
        check("匿名作者".equals(CommunityRequest.authorLabel("")) && "匿名作者".equals(CommunityRequest.authorLabel("  "))
            && "匿名作者".equals(CommunityRequest.authorLabel(null)) && "寻章".equals(CommunityRequest.authorLabel(" 寻章 ")),
            "an item without an author is called 匿名作者, as on the cards");

        // 作品类型与举报同一组值。
        check(List.of("skins", "candidate-skins", "plugins", "dictionaries", "replies", "phrases")
            .equals(CommunityRequest.BLOCK_KINDS), "blocks accept exactly the report kinds");
        for (Kind kind : Kind.values()) {
            check(CommunityRequest.validBlockKind(CommunityRequest.reportKind(kind)),
                "every catalogue kind can be blocked by its report kind: " + kind);
        }
        check(!CommunityRequest.validBlockKind("authors") && !CommunityRequest.validBlockKind("Skins")
            && !CommunityRequest.validBlockKind(null), "other kinds are refused before Rust is called");

        // 一条屏蔽 {id,name,created_at}。
        BlockedAuthor author = CommunityRequest.blockedAuthor(BLOCK, " 合成作者 ", "2026-10-31T23:30:00+08:00");
        check(author != null && BLOCK.equals(author.id()) && "合成作者".equals(author.name())
            && "2026-10-31T23:30:00+08:00".equals(author.createdAt()), "a block record is read with a trimmed name");
        check(CommunityRequest.blockedAuthor(BLOCK, "", "2026-10-31T00:00:00Z") != null,
            "an author without a display name is still a block that can be lifted");
        check(CommunityRequest.blockedAuthor("not-a-uuid", "合成作者", "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor("00000000-0000-0000-0000-000000000000", "合成作者", "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor(42, "合成作者", "2026-10-31T00:00:00Z") == null,
            "a block id must be a non-nil UUID string");
        check(CommunityRequest.blockedAuthor(BLOCK, null, "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor(BLOCK, 7, "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor(BLOCK, "坏\u0000名", "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor(BLOCK, "坏\uD800名", "2026-10-31T00:00:00Z") == null
            && CommunityRequest.blockedAuthor(BLOCK, "x".repeat(129), "2026-10-31T00:00:00Z") == null,
            "a name must be a short single-line string");
        check(CommunityRequest.blockedAuthor(BLOCK, "x".repeat(128), "2026-10-31T00:00:00Z") != null,
            "a name of 128 characters is accepted");
        check(CommunityRequest.blockedAuthor(BLOCK, "合成作者", null) == null
            && CommunityRequest.blockedAuthor(BLOCK, "合成作者", "yesterday") == null
            && CommunityRequest.blockedAuthor(BLOCK, "合成作者", "2026-10-31T00:00:00") == null,
            "created_at must be an RFC 3339 time with an offset");

        // 整份名单。
        BlockedAuthor other = CommunityRequest.blockedAuthor(OTHER, "另一位", "2026-10-01T00:00:00Z");
        check(List.of(author, other).equals(CommunityRequest.blockList(new ArrayList<>(List.of(author, other)))),
            "a list keeps the server's newest-first order");
        check(CommunityRequest.blockList(List.of()).isEmpty(), "an empty list is a valid answer");
        check(CommunityRequest.blockList(Arrays.asList(author, null)) == null,
            "one unreadable row fails the whole list instead of hiding a block");
        check(CommunityRequest.blockList(List.of(author,
                new BlockedAuthor(BLOCK.toUpperCase(java.util.Locale.ROOT), "重复", "2026-10-01T00:00:00Z"))) == null,
            "the same block twice, in any letter case, is a service fault");
        check(CommunityRequest.blockList(null) == null, "a missing list is a service fault");
        List<BlockedAuthor> full = new ArrayList<>(CommunityRequest.MAX_BLOCKED_AUTHORS + 1);
        for (int index = 0; index <= CommunityRequest.MAX_BLOCKED_AUTHORS; index++) {
            full.add(new BlockedAuthor(String.format("40000000-0000-4000-8000-%012d", index + 1), "作者", "2026-10-01T00:00:00Z"));
        }
        check(CommunityRequest.blockList(full) == null, "more than 500 blocks is more than the server keeps");
        check(CommunityRequest.blockList(full.subList(0, CommunityRequest.MAX_BLOCKED_AUTHORS)).size() == 500,
            "500 blocks is the limit itself");

        // 每行的日期按当地时区写成 yyyy-MM-dd。
        check("2026-10-31".equals(CommunityRequest.blockedDate("2026-10-31T23:30:00+08:00", ZoneId.of("Asia/Shanghai"))),
            "a date in the reader's zone");
        check("2026-10-31".equals(CommunityRequest.blockedDate("2026-10-31T15:30:00Z", ZoneId.of("Asia/Shanghai")))
            && "2026-11-01".equals(CommunityRequest.blockedDate("2026-10-31T16:30:00Z", ZoneId.of("Asia/Shanghai"))),
            "a UTC time crosses midnight in the reader's zone");
        check("2026-01-05".equals(CommunityRequest.blockedDate("2026-01-05T00:00:00Z", ZoneId.of("UTC"))),
            "months and days are zero-padded");
        check(CommunityRequest.blockedDate("yesterday", ZoneId.of("UTC")).isEmpty()
            && CommunityRequest.blockedDate(null, ZoneId.of("UTC")).isEmpty()
            && CommunityRequest.blockedDate("2026-10-31T00:00:00Z", null).isEmpty(),
            "an unreadable time leaves the date out");

        // 失败码：Rust 不透传服务端的码，屏蔽自己是 community_invalid，满 500 是 community_conflict。
        check("不能屏蔽自己。".equals(CommunityRequest.blockMessage("community_invalid")), "blocking oneself");
        check("最多屏蔽 500 位作者，请先取消一些。".equals(CommunityRequest.blockMessage("community_conflict")),
            "the 500 limit");
        check(CommunityRequest.blockMessage("community_cancelled").isEmpty()
            && CommunityRequest.blockListMessage("community_cancelled").isEmpty(),
            "a login replaced mid-request is dropped without a message");
        check(CommunityRequest.blockNeedsSignIn("community_unauthorized")
            && !CommunityRequest.blockNeedsSignIn("community_forbidden") && !CommunityRequest.blockNeedsSignIn(null),
            "only an unauthorized answer asks for sign-in");
        check(CommunityRequest.BLOCK_SIGN_IN.equals(CommunityRequest.blockMessage("community_unauthorized"))
            && CommunityRequest.BLOCK_SIGN_IN.equals(CommunityRequest.blockListMessage("community_unauthorized")),
            "signed out reads as the sign-in prompt");
        for (String code : List.of("community_not_found", "community_rate_limited", "community_account_banned",
                "community_unavailable", "community_forbidden")) {
            check(CommunityRequest.rustMessage(code).equals(CommunityRequest.blockMessage(code))
                && CommunityRequest.rustMessage(code).equals(CommunityRequest.blockListMessage(code)),
                "other codes go through the community mapping: " + code);
        }
        check(CommunityRequest.blockMessage(null).equals(CommunityRequest.rustMessage(null))
            && !CommunityRequest.blockMessage(null).isEmpty(), "an unknown failure still says something");
        check(!"不能屏蔽自己。".equals(CommunityRequest.blockListMessage("community_invalid")),
            "only a block can be refused for being one's own");
        System.out.println("Android community blocks: wording, records and failures passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
