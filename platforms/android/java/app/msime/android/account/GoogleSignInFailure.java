package app.msime.android;

import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 把 Credential Manager 的 `NoCredentialException` 说成给人看的一句话。
 *
 * <p>Play services 把每一种不可重试的 `ApiException` 都报成 `NoCredentialException`，只有消息里的状态码能分出是哪一种：`28444` 是这个安装包的签名证书没有登记到 Android OAuth 客户端，`28436` 是用户关掉提示太多次后的冷却期，`28433` 是找不到匹配的凭据。原来除了 `28444` 一律写成「没有可用的 Google 账号」（#5979），屏幕上看不出是哪一种，日志里也没有；现在通用的那句话后面带上状态码。只带数字：消息原文由 Play services 生成，不在屏幕上照搬。
 *
 * <p>不依赖 AndroidX，`check-host.sh` 的 JVM 冒烟 `tests/account/GoogleSignInExplainSmoke.java` 直接调用它。
 */
public final class GoogleSignInFailure {
    /** Play services 的详细状态码写在方括号里，如 `[28436]`。 */
    private static final Pattern BRACKETED = Pattern.compile("\\[(\\d{1,6})]");
    /** 没有方括号时退而取 `ApiException` 消息开头的通用状态码，如 `10: `。 */
    private static final Pattern COMMON = Pattern.compile("(?:^|\\s)(\\d{1,6}):\\s");

    private GoogleSignInFailure() {}

    /** `NoCredentialException` 的那句话；`message` 是异常消息，可以为 null。 */
    public static String noCredential(String message) {
        // 如果 Android OAuth client 没有登记这个包名和签名证书，Play services 同样报告为 no credential，只是消息里带有状态码 `28444`；这种情况下设备上通常是有账号的，提示「没有账号」会把人引到错误的方向。
        if (message != null && (message.contains("28444") || message.contains("Developer console"))) {
            return "Google 登录未配置：这个安装包的签名证书没有登记到 Google Cloud 的 Android OAuth 客户端";
        }
        String code = statusCode(message);
        return "这台设备上没有可用的 Google 账号" + (code.isEmpty() ? "" : "（" + code + "）");
    }

    /** 消息里的状态码，只有数字；方括号里的详细状态码优先，都没有时返回空字符串。 */
    public static String statusCode(String message) {
        if (message == null || message.isEmpty()) return "";
        Matcher bracketed = BRACKETED.matcher(message);
        if (bracketed.find()) return bracketed.group(1);
        Matcher common = COMMON.matcher(message);
        return common.find() ? common.group(1) : "";
    }
}
