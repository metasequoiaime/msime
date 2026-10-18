package app.msime.android;

/**
 * 登录界面上失败时说的那句话，按 Rust 账号入口的错误码（{@link AccountStore.Failure#code}）选（#8201 迁移计划 5b）。
 *
 * <p>以前按 HTTP 状态分：503 是这种登录方式没开放，0 是连不上服务器。Rust 的登录操作把这两种都报成 account_unavailable，所以这里说成「服务暂时不可用，或还没有开放」，不再假装分得清；新加的操作和 link 的几步没拿到响应时另有 account_network。不是 Rust 报的失败（错误码为空串）按「稍后再试」说。
 */
public final class SignInFailurePolicy {
    private SignInFailurePolicy() {}

    /** Google 或 Apple 用凭据换会话失败：`登录没有完成：<原因>`。 */
    public static String login(String code) {
        return "登录没有完成：" + switch (code == null ? "" : code) {
            case "account_network" -> "连不上服务器";
            case "account_rate_limited" -> "尝试太频繁，请稍后再试";
            case "account_unavailable" -> "服务暂时不可用，或这种登录方式还没有开放";
            case "account_unauthorized" -> "服务器拒绝了这次登录";
            case "account_invalid" -> "登录信息无效，请重新开始";
            case "account_recent_login_required" -> "需要先重新登录一次";
            case "account_conflict" -> "这种登录方式已经绑定了别的账号";
            case "account_cancelled" -> "登录已改变，请重新打开页面";
            case "account_storage" -> "本机保存不了登录状态";
            default -> "请稍后再试";
        };
    }

    /** 请服务端发邮箱验证码失败。 */
    public static String emailCode(String code) {
        return switch (code == null ? "" : code) {
            case "account_rate_limited" -> "发送太频繁，请稍后再试";
            case "account_invalid" -> "这个邮箱地址不能用来登录";
            case "account_recent_login_required" -> "需要先重新登录一次";
            case "account_unauthorized" -> "需要先登录水杉账号";
            case "account_network" -> "连不上服务器，请检查网络后再试";
            case "account_unavailable" -> "验证码没有发出：服务暂时不可用，或邮箱登录还没有开放";
            default -> "验证码没有发出，请稍后再试";
        };
    }

    /** 提交邮箱验证码失败。 */
    public static String emailLogin(String code) {
        return switch (code == null ? "" : code) {
            case "account_unauthorized", "account_invalid" -> "验证码不对或已过期";
            case "account_rate_limited" -> "尝试太多次，请稍后再试";
            case "account_recent_login_required" -> "需要先重新登录一次";
            case "account_network" -> "连不上服务器，请检查网络后再试";
            case "account_conflict" -> "这个邮箱已经绑定了别的账号";
            case "account_cancelled" -> "登录已改变，请重新打开页面";
            default -> "登录没有完成，请稍后再试";
        };
    }

    /** 开始 Apple 网页登录（向服务端要授权地址）失败。 */
    public static String appleStart(String code) {
        return switch (code == null ? "" : code) {
            case "account_network" -> "连不上服务器，请检查网络后再试";
            case "account_unavailable" -> "现在无法开始 Apple 登录：服务暂时不可用，或 Apple 登录还没有开放";
            case "account_rate_limited" -> "尝试太频繁，请稍后再试";
            case "account_recent_login_required" -> "需要先重新登录一次";
            case "account_unauthorized" -> "需要先登录水杉账号";
            default -> "现在无法开始 Apple 登录，请稍后再试。";
        };
    }
}
