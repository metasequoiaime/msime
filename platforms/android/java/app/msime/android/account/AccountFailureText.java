package app.msime.android;

/**
 * Rust 账号、社区和 AI 皮肤入口的错误码说成页面上的一句话（#8201 迁移计划 5c）。原来各页按 HTTP 状态和服务端的错误码选文案，现在只拿得到 Rust 的码（`account_*`、`community_*`、`ai_skin_*`、`diagnostics_*`）和可选的 Retry-After 秒数，措辞尽量沿用各页原来的那几句。
 *
 * <p>已有的账号操作（资料、改名、头像、退出）断网时仍是 account_unavailable，只有 #8201 新增的操作才分出 account_network，所以这几处的「暂时不可用」都带上「检查网络」。
 *
 * <p>纯 Java，不碰 org.json，JVM 冒烟（AccountFailureTextSmoke）逐条核对。
 */
public final class AccountFailureText {
    private AccountFailureText() {}

    /** Retry-After 的秒数说成「N 秒」「N 分钟」「N 小时」，向上取整，与 iOS 的 `BackendRustClient.Failure.wait` 相同（Java 里 wait 是 Object 的方法，所以叫 duration）。 */
    public static String duration(long seconds) {
        long value = BoundsPolicy.nonNegative(seconds);
        if (value < 60) return Math.max(1, value) + " 秒";
        if (value < 3600) return ((value + 59) / 60) + " 分钟";
        return ((value + 3599) / 3600) + " 小时";
    }

    /** 「约 N 后再试」，没有 Retry-After 时是「稍后再试」。 */
    static String later(long retryAfterSeconds) {
        return retryAfterSeconds > 0 ? "约 " + duration(retryAfterSeconds) + "后再试" : "稍后再试";
    }

    /** 几处页面共用的审核与封禁文案，与社区页（{@link CommunityRequest#message}）相同。 */
    private static String moderation(String code) {
        return switch (code) {
            case "account_blocked_content", "community_blocked_content" -> "内容包含不允许发布的词语，请修改后再提交";
            case "account_screening_unavailable", "community_screening_unavailable" -> "审核服务暂时不可用，请稍后重试";
            case "account_banned", "community_account_banned" -> "该账号已被封禁，暂时无法使用账号相关功能";
            default -> null;
        };
    }

    /** 个人资料页的动作（改名、头像、云端数据、导出、注销）。 */
    public static String profile(String code, long retryAfterSeconds) {
        String moderated = moderation(code);
        if (moderated != null) return moderated;
        return switch (code) {
            case "account_network" -> "连不上服务器，请检查网络后再试";
            case "account_cancelled" -> "登录已改变，请重新打开页面";
            case "account_unauthorized" -> "登录已失效，请重新登录";
            case "account_rate_limited" -> "操作太频繁，请" + later(retryAfterSeconds);
            case "account_too_large" -> "图片超过 1 MB，请换一张小一些的";
            case "account_recent_login_required" -> "需要重新登录后再试";
            case "account_storage" -> "读不到本机的登录状态，请重新登录";
            case "account_unavailable" -> "暂时没有完成，请检查网络后再试";
            default -> "没有完成，请稍后再试";
        };
    }

    /** 我的设备：读列表与移除一台。 */
    public static String devices(String code, boolean removing) {
        return switch (code) {
            case "account_unauthorized" -> removing ? "登录已失效，请重新登录" : "登录后可以在这里查看和移除登录过的设备。";
            case "account_network" -> removing ? "连不上服务器，请检查网络后再试" : "连不上服务器，请检查网络后再试。";
            case "account_cancelled" -> "登录已改变，请重新打开设备列表";
            case "account_not_found" -> "那台设备已经退出了，请刷新后再看";
            default -> removing ? "没有移除，请稍后再试" : "设备列表暂时读不到，请稍后再试。";
        };
    }

    /** 帮助与反馈的提交。 */
    public static String feedback(String code, long retryAfterSeconds) {
        String moderated = moderation(code);
        if (moderated != null) return moderated;
        return switch (code) {
            case "account_rate_limited" -> "提交得太频繁了，请" + later(retryAfterSeconds);
            case "account_unavailable" -> "反馈服务暂时不可用，可以改用下面的渠道";
            case "account_network" -> "连不上服务器，请检查网络后重试";
            case "account_too_large" -> "截图太大，请换一张小一些的";
            case "account_invalid" -> "描述或截图不符合要求，请检查后重试";
            default -> "没有提交成功，请稍后重试";
        };
    }

    /** 开发者选项里的 MCP 日志上传、查看、换令牌和删除。 */
    public static String diagnostics(String code, long retryAfterSeconds) {
        if (code.startsWith("diagnostics_")) return "诊断包读取失败，请重试";
        return switch (code) {
            case "account_network" -> "连不上水杉云，请检查网络";
            case "account_rate_limited" -> "上传太频繁，请" + later(retryAfterSeconds);
            case "account_too_large" -> "日志太大，请少选几类再上传";
            case "account_unauthorized" -> "账号会话已失效，请稍后重试";
            case "account_identity_changed", "account_cancelled" -> "账号已经变了，请重新打开本页";
            case "account_not_found" -> "云端已经没有这份日志快照了";
            case "account_invalid" -> "日志未通过校验，未上传";
            case "account_unavailable" -> "这项服务暂时不可用，请稍后再试";
            default -> "操作失败，请重试";
        };
    }

    /** 其他平台下载的「发送链接」。 */
    public static String downloadLink(String code, long retryAfterSeconds) {
        return switch (code) {
            case "account_no_verified_email" -> "账号还没有验证过的邮箱，请改用「复制链接」";
            case "account_unauthorized" -> "登录后才能发到邮箱，也可以改用「复制链接」";
            case "account_rate_limited" -> "发送得太频繁了，请" + later(retryAfterSeconds);
            case "account_unavailable" -> "暂时不能发送邮件，请改用「复制链接」";
            case "account_network" -> "连不上服务器，请检查网络后重试";
            case "account_cancelled" -> "登录已改变，请重新打开页面";
            default -> "没有发送成功，请改用「复制链接」";
        };
    }

    /** AI 对话的一次失败；account_cancelled 由页面自己处理（丢掉这次回答），不经这里。 */
    public static String chat(String code, long retryAfterSeconds) {
        return switch (code) {
            case "account_rate_limited" -> "请求太频繁，请" + later(retryAfterSeconds) + "。";
            case "account_network" -> "连接失败，请检查网络后重试。";
            case "account_too_large" -> "对话太长了，清空后再试。";
            case "account_unauthorized" -> "请先登录后再和 AI 对话。";
            default -> "请求失败，请检查登录状态或稍后重试。";
        };
    }

    /**
     * 这个部署是不是没开 AI 皮肤、入口要藏起来：只看没有 Retry-After 的 account_unavailable。服务端任务繁忙的 503（skin_jobs_busy）也是 account_unavailable，但带 retry_after_seconds，只是一时忙，不能把入口藏掉一天（decisions-followup 决定 17 的「503 的细分」）。
     *
     * <p>Rust 的契约分不出「没开 AI 皮肤的 503 job_unavailable」和下面几种，它们也是不带 Retry-After 的 account_unavailable，同样会藏入口一天（原来的 Java 只看 503）：访问令牌要先刷新、而刷新请求本身没拿到响应（例如登录账号的 15 分钟令牌过期后断网点「生成」），其他 5xx（例如一时的 500、502），重定向被拒。生成前注册匿名账号失败不在其中，见 {@link #aiSkinAnonymousFailure}。
     */
    public static boolean aiSkinUnavailable(String code, long retryAfterSeconds) {
        return "account_unavailable".equals(code) && retryAfterSeconds <= 0;
    }

    /** 没登录、也还没有匿名账号时，AI 皮肤生成前注册匿名账号没有成功；只在 Java 里出现，不是 Rust 的码。 */
    public static final String AI_SKIN_ANONYMOUS_UNAVAILABLE = "ai_skin_anonymous_unavailable";

    /**
     * AI 皮肤生成前注册匿名账号（ensure_anonymous）失败时交给页面的码。ensure_anonymous 是已有操作，断网和服务端 5xx 都是不带 Retry-After 的 account_unavailable，{@link RustAccount#ensureAnonymous} 的兜底也映射成它；这与「部署没开 AI 皮肤」无关，所以换成 {@link #AI_SKIN_ANONYMOUS_UNAVAILABLE}，不让页面把入口藏一天。其他码（限流、封禁等）原样交回。
     */
    public static String aiSkinAnonymousFailure(String code) {
        return "account_unavailable".equals(code) ? AI_SKIN_ANONYMOUS_UNAVAILABLE : code;
    }

    /** AI 设计皮肤的一次失败；ai_skin_cancelled 由页面自己处理（不提示），不经这里。 */
    public static String aiSkin(String code, long retryAfterSeconds) {
        if (aiSkinUnavailable(code, retryAfterSeconds)) return "AI 设计皮肤暂不可用";
        return switch (code) {
            case "account_rate_limited" -> retryAfterSeconds >= 3600
                ? "今天的生成次数已用完，约 " + duration(retryAfterSeconds) + "后可以再试"
                : "暂时不能生成，请" + later(retryAfterSeconds);
            case "account_unavailable" -> "生成任务繁忙，请" + later(retryAfterSeconds);
            case AI_SKIN_ANONYMOUS_UNAVAILABLE -> "暂时连不上服务，请检查网络后重试";
            // 描述在交给 Rust 之前已经按 1–500 个字、不含控制字符查过，这时的 ai_skin_invalid 是模型给出的方案或插画不合规。
            case "ai_skin_invalid" -> "这次的设计不合格，换个描述再试一次";
            case "ai_skin_busy" -> "上一次生成还没结束，请稍候";
            case "account_cancelled" -> "登录已改变，这次生成的结果没有保留";
            case "account_network" -> "连不上服务器，请检查网络后重试";
            case "account_unauthorized" -> "请先登录后再生成";
            default -> "生成失败，请稍后重试";
        };
    }

    /** 云端数据汇总的 `anonymous_voice` 是 unknown 时，语音样本那一行的说明；其余（included、none）为 null，照常显示条数。 */
    public static String anonymousVoice(String anonymousVoice) {
        return "unknown".equals(anonymousVoice) ? "语音样本数暂不可知" : null;
    }

    /**
     * 注销账号或删除云端数据成功以后，删本机匿名账号名下语音样本的那一步没做成（`voice_samples.ok` 为 false）：前面的删除已经生效，只提示样本没有删掉、到「语音」里重试。
     */
    public static String voiceSamplesLeft(String code, long retryAfterSeconds) {
        String retry = "account_rate_limited".equals(code) ? "约 " + duration(Math.max(retryAfterSeconds, 1)) + "后" : "稍后";
        return "上传过的语音样本没有删掉，可以" + retry + "在「语音」设置里再删一次";
    }

    /** 「语音」设置里的「删除上传过的语音样本」（voice_delete）失败。 */
    public static String voiceDelete(String code, long retryAfterSeconds) {
        return switch (code) {
            case "account_rate_limited" -> "删除太频繁，请" + later(retryAfterSeconds);
            case "account_network" -> "连不上服务器，请检查网络后重试";
            default -> "没有删掉，请稍后重试";
        };
    }

    /** 退出登录时 logout 没做成：本机已经退出（Rust 先清本地会话），只是服务端的这次登录没有撤销。 */
    public static String signedOutLocally(String code) {
        return "account_network".equals(code) || "account_unavailable".equals(code)
            ? "已在本机退出登录；连不上服务器，这台设备的登录要到过期后才在服务端失效"
            : "已在本机退出登录；服务端没有撤销这次登录，它会在过期后失效";
    }
}
