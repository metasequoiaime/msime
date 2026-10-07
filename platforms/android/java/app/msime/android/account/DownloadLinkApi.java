package app.msime.android;

import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「其他平台下载」的「发送链接」：`POST /v1/users/me/download-link`，服务端把某个平台的下载页链接发到这个账号已经验证过的邮箱。
 *
 * <p>收件地址由服务端决定，客户端给不了，所以这个接口不会被拿来往任意地址发信；没有已验证邮箱的账号收到 409 `no_verified_email`，页面改为提示「复制链接」。只有真实账号有邮箱，匿名账号不调这个接口。限流每用户每小时 3 次、每天 10 次。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class DownloadLinkApi {
    public static final String PATH = "/v1/users/me/download-link";
    /** 服务端认的平台取值。 */
    public static final List<String> PLATFORMS = List.of("windows", "macos", "linux", "harmony-pc", "ios", "android", "harmony");
    /** 没有已验证邮箱时服务端给的错误码。 */
    public static final String NO_VERIFIED_EMAIL = "no_verified_email";

    private final CloudApi api;

    public DownloadLinkApi(CloudApi api) {
        this.api = api;
    }

    /** 失败是不是因为账号没有已验证的邮箱。 */
    public static boolean noVerifiedEmail(CloudApi.Failure failure) {
        return failure != null && failure.status == 409 && NO_VERIFIED_EMAIL.equals(failure.code);
    }

    /** 发送链接，返回服务端打码后的收件地址（例如 `u***@example.com`），读不到时为空字符串。 */
    public String send(String platform) throws CloudApi.Failure {
        if (!PLATFORMS.contains(platform)) throw new IllegalArgumentException("unknown platform " + platform);
        JSONObject body;
        try {
            body = new JSONObject().put("platform", platform);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        JSONObject response = api.json("POST", PATH, body, CloudApi.Auth.ACCOUNT);
        Object sentTo = response.opt("sent_to");
        return sentTo instanceof String text && text.length() <= 320 && !TextPolicy.hasControl(text)
            && TextPolicy.validUnicode(text) ? text : "";
    }
}
