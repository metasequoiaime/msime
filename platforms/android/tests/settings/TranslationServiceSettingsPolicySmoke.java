import app.msime.android.TranslationServiceSettingsPolicy;
import app.msime.android.TranslationServiceSettingsPolicy.Choice;

/** 「翻译服务」设置页：选中项的判定与共享偏好的字段校验一致，不合规的值在输入框里就被拦下并说明原因。 */
public final class TranslationServiceSettingsPolicySmoke {
    public static void main(String[] args) {
        // 共享偏好的默认值：腾讯启用但没有凭据，页面显示「不使用」，而不是一个用户没选过的腾讯。
        check(TranslationServiceSettingsPolicy.selected(false, false, false, true, "", "") == Choice.NONE,
            "default preferences show 不使用");
        check(TranslationServiceSettingsPolicy.selected(true, false, false, true, "", "") == Choice.ACCOUNT,
            "an explicit account choice shows 水杉账号");
        check(TranslationServiceSettingsPolicy.selected(true, true, false, true, "", "") == Choice.NIUTRANS,
            "an enabled NiuTrans shows even with incomplete credentials");
        check(TranslationServiceSettingsPolicy.selected(false, false, true, false, "", "") == Choice.CUSTOM,
            "an enabled custom service shows even without an endpoint");
        check(TranslationServiceSettingsPolicy.selected(true, false, false, true, "AKIDsynthetic", "synthetic")
                == Choice.TENCENT, "usable Tencent credentials show 腾讯云机器翻译");
        check(TranslationServiceSettingsPolicy.selected(false, false, false, true, "<SecretId>", "synthetic")
                == Choice.NONE, "a Tencent placeholder is not a choice");

        check(Choice.NONE.testService() == null && Choice.ACCOUNT.testService() == null,
            "neither 不使用 nor 水杉账号 has a connection test");
        check("translation.custom".equals(Choice.CUSTOM.testService())
                && "translation.niutrans".equals(Choice.NIUTRANS.testService())
                && "translation.tencent".equals(Choice.TENCENT.testService()),
            "the connection test names the shared credential services");

        check(TranslationServiceSettingsPolicy.niutransProblem("") == null
                && TranslationServiceSettingsPolicy.niutransProblem("app-id") == null,
            "NiuTrans accepts an empty or real value");
        check(TranslationServiceSettingsPolicy.niutransProblem("<YOUR_NIUTRANS_APIKEY>") != null
                && TranslationServiceSettingsPolicy.niutransProblem("FAKESECRET_key") != null,
            "NiuTrans rejects placeholders the shared preferences refuse");
        check(TranslationServiceSettingsPolicy.secretIdProblem("AKID_synthetic-1") == null
                && TranslationServiceSettingsPolicy.secretIdProblem("") == null,
            "a Tencent SecretId is an ASCII identifier");
        check(TranslationServiceSettingsPolicy.secretIdProblem("AKID synthetic") != null
                && TranslationServiceSettingsPolicy.secretIdProblem("秘密") != null,
            "a SecretId with other characters is refused");
        check(TranslationServiceSettingsPolicy.regionProblem("ap-guangzhou") == null
                && TranslationServiceSettingsPolicy.regionProblem("") == null,
            "a Tencent region is letters, digits and dashes");
        check(TranslationServiceSettingsPolicy.regionProblem("ap_guangzhou") != null
                && TranslationServiceSettingsPolicy.regionProblem("a".repeat(65)) != null,
            "a region with underscores or over 64 characters is refused");
        check(TranslationServiceSettingsPolicy.secretProblem("synthetic") == null
                && TranslationServiceSettingsPolicy.secretProblem("a\u0000b") != null
                && TranslationServiceSettingsPolicy.secretProblem("x".repeat(4097)) != null,
            "secrets are bounded and free of control characters");
        check(TranslationServiceSettingsPolicy.endpointProblem("") == null
                && TranslationServiceSettingsPolicy.endpointProblem("https://api-free.deepl.com/v2/translate") == null
                && TranslationServiceSettingsPolicy.endpointProblem("http://127.0.0.1:1188/translate") == null,
            "https and loopback http endpoints are accepted");
        String lan = TranslationServiceSettingsPolicy.endpointProblem("http://192.168.1.20:1188/translate");
        check(lan != null && lan.contains("HTTPS"), "a LAN http endpoint is refused with the reason");
        check(TranslationServiceSettingsPolicy.endpointProblem("https://user:secret@translate.example/api") != null,
            "an endpoint with credentials is refused");
        check(TranslationServiceSettingsPolicy.endpointProblem("https://my_host.example.com/translate") == null
                && TranslationServiceSettingsPolicy.endpointProblem("https://例子.example/translate") == null
                && TranslationServiceSettingsPolicy.endpointProblem("https://deeplx.example.com/translate?token=a|b") == null,
            "https endpoints the shared WHATWG parser accepts are not refused locally");
        check(TranslationServiceSettingsPolicy.endpointProblem("https://translate.example/api", true, "") == null,
            "the shared layer's acceptance decides");
        check("远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址。".equals(TranslationServiceSettingsPolicy.endpointProblem(
                "http://192.168.1.20/translate", false, "远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址。")),
            "a refusal shows the shared layer's hint");
        check(TranslationServiceSettingsPolicy.endpointProblem("https:///v1", false, "") != null,
            "a refusal without a shared hint still names the problem");
        check(TranslationServiceSettingsPolicy.endpointProblem("https://my_host.example.com/translate", null, "") == null
                && TranslationServiceSettingsPolicy.endpointProblem("http://192.168.1.20/translate", null, "") != null,
            "without the shared library the local rule applies");
        System.out.println("Android translation service settings: selection and field rules passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
