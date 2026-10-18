package app.msime.android;

/** Rust 账号、社区和 AI 皮肤入口的错误码在各页上说成的那句话（#8201 迁移计划 5c）。 */
public final class AccountFailureTextSmoke {
    public static void main(String[] arguments) {
        check("1 秒".equals(AccountFailureText.duration(0)) && "59 秒".equals(AccountFailureText.duration(59)),
            "seconds, at least one");
        check("1 分钟".equals(AccountFailureText.duration(60)) && "2 分钟".equals(AccountFailureText.duration(61)),
            "minutes round up");
        check("2 小时".equals(AccountFailureText.duration(3601)), "hours round up");

        // AI 皮肤只在不带 Retry-After 的 account_unavailable 时藏入口；任务一时繁忙的 503 带着 Retry-After。
        check(AccountFailureText.aiSkinUnavailable("account_unavailable", 0), "a deployment without AI skins hides the entry");
        check(!AccountFailureText.aiSkinUnavailable("account_unavailable", 5), "busy jobs do not hide the entry");
        check(!AccountFailureText.aiSkinUnavailable("account_rate_limited", 0), "a quota does not hide the entry");
        check(AccountFailureText.aiSkin("account_unavailable", 5).contains("繁忙"), "busy jobs ask for a retry");
        // 没登录又没有匿名账号时，生成前注册匿名账号失败（断网时也是不带 Retry-After 的 account_unavailable）不藏入口。
        String anonymous = AccountFailureText.aiSkinAnonymousFailure("account_unavailable");
        check(!AccountFailureText.aiSkinUnavailable(anonymous, 0), "a failed anonymous registration does not hide the entry");
        check(AccountFailureText.aiSkin(anonymous, 0).contains("检查网络"), "a failed anonymous registration asks to check the network");
        check("account_rate_limited".equals(AccountFailureText.aiSkinAnonymousFailure("account_rate_limited"))
            && "account_banned".equals(AccountFailureText.aiSkinAnonymousFailure("account_banned")),
            "other registration failures keep their codes");
        check(AccountFailureText.aiSkin("account_rate_limited", 7200).equals("今天的生成次数已用完，约 2 小时后可以再试"),
            "a daily quota with Retry-After");
        check(AccountFailureText.aiSkin("account_rate_limited", 30).contains("约 30 秒后再试"),
            "a short wait is not called the daily quota");
        check(AccountFailureText.aiSkin("ai_skin_invalid", 0).contains("不合格"),
            "after local prompt checks, ai_skin_invalid is the model's design");
        check(AccountFailureText.aiSkin("account_network", 0).startsWith("连不上服务器"), "network");
        check(AccountFailureText.aiSkin("account_cancelled", 0).contains("登录已改变"), "a replaced login");

        // 云端数据汇总合不进匿名账号的语音样本时写「暂不可知」，合进了或本来没有就照常显示条数。
        check("语音样本数暂不可知".equals(AccountFailureText.anonymousVoice("unknown")), "unknown anonymous voice");
        check(AccountFailureText.anonymousVoice("included") == null && AccountFailureText.anonymousVoice("none") == null
            && AccountFailureText.anonymousVoice("") == null, "known voice counts need no note");
        check(AccountFailureText.voiceSamplesLeft("account_rate_limited", 3600).contains("约 1 小时后"),
            "voice samples left behind say when to retry");
        check(AccountFailureText.voiceSamplesLeft("account_network", 0).contains("语音"),
            "voice samples left behind point to the voice settings");
        check(AccountFailureText.voiceDelete("account_rate_limited", 60).contains("约 1 分钟后再试"), "voice delete quota");

        // 退出登录时服务端那一步没做成：本机已经退出，只说服务端没有撤销。
        check(AccountFailureText.signedOutLocally("account_unavailable").startsWith("已在本机退出登录"),
            "an offline logout still signs out here");
        check(AccountFailureText.signedOutLocally("account_unauthorized").startsWith("已在本机退出登录"),
            "a revoked session still signs out here");

        check(AccountFailureText.profile("account_rate_limited", 120).contains("约 2 分钟后再试"), "profile rate limit");
        check(AccountFailureText.profile("account_unavailable", 0).contains("检查网络"),
            "old operations report offline as unavailable, so the wording mentions the network");
        check(AccountFailureText.profile("account_blocked_content", 0).contains("不允许发布"), "a refused nickname");
        check(AccountFailureText.devices("account_unauthorized", false).startsWith("登录后"), "devices need a login");
        check(AccountFailureText.devices("account_network", true).startsWith("连不上服务器"), "devices offline");
        check(AccountFailureText.feedback("account_too_large", 0).contains("截图"), "an oversized screenshot");
        check(AccountFailureText.feedback("account_rate_limited", 0).endsWith("稍后再试"), "feedback without Retry-After");
        check(AccountFailureText.diagnostics("diagnostics_unreadable", 0).contains("诊断包"), "local diagnostics records");
        check(AccountFailureText.diagnostics("account_identity_changed", 0).contains("账号已经变了"),
            "a snapshot bound to another account");
        check(AccountFailureText.downloadLink("account_no_verified_email", 0).contains("复制链接"), "no verified email");
        check(AccountFailureText.chat("account_too_large", 0).contains("清空"), "an overlong conversation");
        check(AccountFailureText.chat("account_unauthorized", 0).contains("登录"), "chat needs a login");
        System.out.println("Android account failure wording passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
