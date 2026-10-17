import app.msime.android.CloudApi;
import app.msime.android.TypingUploadApi;
import java.time.ZoneId;
import java.util.ArrayList;
import java.util.List;

/** 「上传到云端」和「绑定 QQ 群助手」的请求路径、绑定码规则、时间显示和错误说法。带 JSON 的请求（上传、列表、绑定）要真实的 org.json，这里只有 android.jar 的桩，由设备和服务端的测试覆盖。 */
public final class TypingUploadApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 绑定码：去掉空白和连字符后正好 8 位 ASCII 数字，和服务端、共享设置页一致。
        check("12345678".equals(TypingUploadApi.normalizeCode("12345678")), "plain code");
        check("12345678".equals(TypingUploadApi.normalizeCode(" 1234-5678 ")), "spaces and dashes are dropped");
        check(TypingUploadApi.normalizeCode("1234567") == null, "seven digits");
        check(TypingUploadApi.normalizeCode("123456789") == null, "nine digits");
        check(TypingUploadApi.normalizeCode("1234567a") == null, "letters");
        check(TypingUploadApi.normalizeCode("１２３４５６７８") == null, "full-width digits are not ASCII");
        check(TypingUploadApi.normalizeCode(null) == null, "null");

        // 时间按本机时区显示；读不懂的原样返回。
        ZoneId tokyo = ZoneId.of("Asia/Tokyo");
        check("2026-10-17 12:34".equals(TypingUploadApi.displayTime("2026-10-17T03:34:56Z", tokyo)), "UTC to local");
        check("2026-10-17 12:34".equals(TypingUploadApi.displayTime("2026-10-17T11:34:56+08:00", tokyo)), "offset to local");
        check("昨天".equals(TypingUploadApi.displayTime("昨天", tokyo)), "unreadable time is kept");
        check("2026-10-17 12:34".equals(TypingUploadApi.displayMillis(1_792_208_096_000L, tokyo)), "upload millis to local");

        // 错误说法与共享设置页相同。
        check(TypingUploadApi.linkMessage(failure(404, "qq_link_code_not_found")).startsWith("绑定码不对或已过期"), "expired code");
        check(TypingUploadApi.linkMessage(failure(409, "qq_link_limit")).startsWith("这个账号已经绑定了 10 个"), "limit");
        check(TypingUploadApi.linkMessage(failure(400, "invalid_qq_link_code")).startsWith("绑定码是 8 位数字"), "invalid code");
        check(TypingUploadApi.linkMessage(failure(429, "rate_limit_exceeded")).startsWith("尝试次数太多"), "rate limited");
        check(TypingUploadApi.linkMessage(failure(0, "network")).startsWith("连不上服务器"), "network");
        check(TypingUploadApi.uploadMessage(failure(401, "invalid_credentials"), true).startsWith("先在「我的」页登录"), "signed out");
        check(TypingUploadApi.uploadMessage(failure(0, "network"), true).startsWith("已打开上传，但这次没有传上去"), "enable failed");
        check(TypingUploadApi.uploadMessage(failure(503, "auth_unavailable"), false).startsWith("已关闭上传，但云端的数据没有删掉"), "delete failed");
        check(TypingUploadApi.generalMessage(failure(404, "")).startsWith("这项服务暂时不可用"), "old server");

        // 删除走真实账号的令牌、固定路径；绑定码不合规时不联网。
        List<String> seen = new ArrayList<>();
        CloudApi api = new CloudApi((method, path, headers, body) -> {
            seen.add(method + " " + path + " " + headers.get("Authorization"));
            return new CloudApi.Exchange(204, null, null, new byte[0]);
        }, rejected -> "a".repeat(64), rejected -> "b".repeat(64));
        TypingUploadApi upload = new TypingUploadApi(api);
        upload.deleteAll();
        upload.remove("42");
        check(seen.equals(List.of("DELETE /v1/users/me/typing-statistics Bearer " + "a".repeat(64),
            "DELETE /v1/users/me/qq-links/42 Bearer " + "a".repeat(64))), "delete paths and account token: " + seen);
        try {
            upload.redeem("1234");
            throw new AssertionError("a short code was sent");
        } catch (CloudApi.Failure expected) {
            check(expected.status == 400 && "invalid_qq_link_code".equals(expected.code), "short code is refused locally");
        }
        for (String id : new String[] {"", "0", "042", "4a", "../x", "12345678901234567890"}) {
            try {
                upload.remove(id);
                throw new AssertionError("an invalid id was sent: " + id);
            } catch (IllegalArgumentException expected) {
                // 编号不合规时不拼进路径。
            }
        }
        try {
            upload.upload("/v1/users/me/phrases", null);
            throw new AssertionError("an unexpected upload path was sent");
        } catch (IllegalArgumentException expected) {
            // 只发共享层交回的那个路径。
        }
        check(seen.size() == 2, "nothing else reached the network");
        System.out.println("Android typing upload API: paths, bind codes, times and messages passed");
    }

    private static CloudApi.Failure failure(int status, String code) {
        return new CloudApi.Failure(status, code, "synthetic", 0);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
