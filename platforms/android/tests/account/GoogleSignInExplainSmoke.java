import app.msime.android.GoogleSignInFailure;

public final class GoogleSignInExplainSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        String generic = "这台设备上没有可用的 Google 账号";
        String unconfigured = GoogleSignInFailure.noCredential(
            "During get sign-in intent, failure response from one tap: 10: [28444] Developer console is not set up correctly.");
        check(unconfigured.startsWith("Google 登录未配置"), "28444 is a configuration failure: " + unconfigured);
        check(GoogleSignInFailure.noCredential("Developer console is not set up correctly.").startsWith("Google 登录未配置"),
            "the Developer console text alone is a configuration failure");

        // 冷却期（`28436`）和找不到匹配凭据（`28433`）都报成 no credential；屏幕上那句话要带着状态码，只带数字，不带 Play services 的原文。
        String cooldown = GoogleSignInFailure.noCredential(
            "During begin sign in, failure response from one tap: 16: [28436] Caller has been temporarily blocked due to too many canceled sign-in prompts.");
        check(cooldown.equals(generic + "（28436）"), "bracketed status code is shown: " + cooldown);
        check(!cooldown.contains("blocked"), "Play services text is not shown: " + cooldown);
        check(GoogleSignInFailure.noCredential("[28433] Cannot find a matching credential.").equals(generic + "（28433）"),
            "leading bracketed status code is shown");
        check(GoogleSignInFailure.noCredential("During get sign-in intent, failure response from one tap: 8: internal error")
            .equals(generic + "（8）"), "plain common status code is shown when there is no bracketed one");

        check(GoogleSignInFailure.noCredential(null).equals(generic), "null message is the generic text");
        check(GoogleSignInFailure.noCredential("").equals(generic), "empty message is the generic text");
        check(GoogleSignInFailure.noCredential("No credentials available").equals(generic), "message without a code is the generic text");
        check(GoogleSignInFailure.noCredential("version 1.6.0 of something").equals(generic), "a version number is not a status code");

        check(GoogleSignInFailure.statusCode("16: [28436] blocked").equals("28436"), "bracketed code wins over the common code");
        check(GoogleSignInFailure.statusCode("[1234567] too long").isEmpty(), "overlong numbers are not status codes");
        System.out.println("Android Google sign-in failure text passed");
    }
}
