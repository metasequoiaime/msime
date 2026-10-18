package app.msime.android;

import android.content.Context;
import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「其他平台下载」的「发送链接」：Rust 的 {@code msime_client_account} 的 download_link（#8201 迁移计划 5c），服务端把某个平台的下载页链接发到这个账号已经验证过的邮箱。
 *
 * <p>收件地址由服务端决定，客户端给不了，所以这个接口不会被拿来往任意地址发信；没有已验证邮箱的账号是 account_no_verified_email，页面改为提示「复制链接」。只用登录账号。限流每用户每小时 3 次、每天 10 次。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class DownloadLinkApi {
    /** 服务端认的平台取值。 */
    public static final List<String> PLATFORMS = List.of("windows", "macos", "linux", "harmony-pc", "ios", "android", "harmony");

    private DownloadLinkApi() {}

    /**
     * 给页面打开时登录的那个账号（`expectedUserId`）发送链接，返回服务端打码后的收件地址（例如 `u***@example.com`），没有时为空字符串。请求前后登录的不是它时以 account_cancelled 失败。
     */
    public static String send(Context context, String expectedUserId, String platform) throws AccountStore.Failure {
        if (!PLATFORMS.contains(platform)) throw new AccountStore.Failure("account_invalid", 0);
        final JSONObject action;
        try {
            action = new JSONObject().put("operation", "download_link").put("platform", platform);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        RustAccount.requireUser(context, expectedUserId);
        Object value = RustAccount.account(context, action);
        RustAccount.requireUser(context, expectedUserId);
        return value instanceof JSONObject result ? JsonPolicy.strictStringOrEmpty(result.opt("sent_to")) : "";
    }
}
