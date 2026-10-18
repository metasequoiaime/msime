import app.msime.android.SignInFailurePolicy;

/** 登录界面按 Rust 账号入口的错误码说话（#8201 迁移计划 5b）。 */
public final class SignInFailurePolicySmoke {
    public static void main(String[] args) {
        check(SignInFailurePolicy.login("account_network").equals("登录没有完成：连不上服务器"),
            "a request that got no response says the server could not be reached");
        check(SignInFailurePolicy.login("account_unavailable").contains("服务暂时不可用")
                && SignInFailurePolicy.login("account_unavailable").contains("还没有开放"),
            "account_unavailable covers both an outage and a provider that is not enabled");
        check(SignInFailurePolicy.login("account_rate_limited").contains("太频繁"), "rate limits ask to wait");
        check(SignInFailurePolicy.login("account_conflict").contains("绑定了别的账号"),
            "a provider already linked elsewhere is named");
        check(SignInFailurePolicy.login("account_recent_login_required").contains("重新登录"),
            "a stale session asks for a fresh sign-in");
        check(SignInFailurePolicy.login("").equals("登录没有完成：请稍后再试"),
            "a failure that is not from Rust asks for a retry");
        check(SignInFailurePolicy.login(null).equals(SignInFailurePolicy.login("")), "a missing code is not a crash");

        check(SignInFailurePolicy.emailCode("account_rate_limited").equals("发送太频繁，请稍后再试"),
            "sending codes too often is named");
        check(SignInFailurePolicy.emailCode("account_invalid").equals("这个邮箱地址不能用来登录"),
            "a refused address is named");
        check(SignInFailurePolicy.emailCode("account_network").contains("连不上服务器"), "offline is named");
        check(SignInFailurePolicy.emailCode("account_unavailable").startsWith("验证码没有发出"),
            "an outage still says the code was not sent");

        check(SignInFailurePolicy.emailLogin("account_unauthorized").equals("验证码不对或已过期"),
            "a wrong code is a wrong code");
        check(SignInFailurePolicy.emailLogin("account_invalid").equals("验证码不对或已过期"),
            "an expired challenge reads the same as a wrong code");
        check(SignInFailurePolicy.emailLogin("account_rate_limited").contains("尝试太多次"), "too many tries");
        check(SignInFailurePolicy.emailLogin("account_cancelled").contains("登录已改变"),
            "an account switch during a link is named");
        check(SignInFailurePolicy.emailLogin("account_unavailable").equals("登录没有完成，请稍后再试"),
            "an outage asks for a retry");

        check(SignInFailurePolicy.appleStart("account_network").contains("连不上服务器"), "Apple start offline");
        check(SignInFailurePolicy.appleStart("account_unavailable").contains("Apple 登录还没有开放"),
            "Apple start covers a provider that is not enabled");
        check(SignInFailurePolicy.appleStart("account_invalid").equals("现在无法开始 Apple 登录，请稍后再试。"),
            "other Apple start failures ask for a retry");
        System.out.println("Android sign-in failure wording passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
