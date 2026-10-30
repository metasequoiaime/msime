import app.msime.android.CandidateTranslationPolicy;
import app.msime.android.CandidateTranslationRoute;
import app.msime.android.CandidateTranslationRoute.Provider;

/**
 * 候选翻译选路对照共享层 `crates/host-api/src/ffi/providers.rs` 的 `selected_translation_services`：选路是 Java 对 Rust 的镜像，Rust 改了优先级而这里没跟上时由这张表拦下。
 */
public final class CandidateTranslationRouteSmoke {
    /** 一行偏好：候选翻译开关、水杉账号，再是小牛、自定义、腾讯各自的启用和凭据。 */
    private record Row(String name, boolean translations, boolean account,
            boolean niutrans, String appId, String niutransKey,
            boolean custom, String endpoint, String customKey,
            boolean tencent, String secretId, String secretKey, String region, Provider expected) {
        CandidateTranslationRoute route() {
            return CandidateTranslationPolicy.route(translations, account, niutrans, appId, niutransKey,
                custom, endpoint, customKey, tencent, secretId, secretKey, region);
        }
    }

    public static void main(String[] args) throws Exception {
        // AI 接口地址的结论来自 Rust，JVM 里没有 JNI，按核对过的表作答。
        app.msime.android.AiEndpointContract.install();
        Row[] rows = {
            // 共享偏好的默认值：候选翻译开着、腾讯默认启用但没有凭据，所以什么都不发。
            new Row("default preferences", true, false, false, "", "", false, "", "",
                true, "", "", "ap-guangzhou", Provider.NONE),
            new Row("explicit account", true, true, false, "", "", false, "", "",
                true, "", "", "ap-guangzhou", Provider.ACCOUNT),
            new Row("NiuTrans with credentials", true, false, true, "app", "key", false, "", "",
                false, "", "", "", Provider.NIUTRANS),
            new Row("NiuTrans without a key never falls back", true, true, true, "app", "", true,
                "https://translate.example/api", "", true, "AKIDsynthetic", "synthetic", "", Provider.NONE),
            new Row("custom without a key", true, false, false, "", "", true,
                "http://127.0.0.1:1188/translate", "", false, "", "", "", Provider.CUSTOM),
            new Row("custom with a blank endpoint", true, false, false, "", "", true, " \t ", "key",
                false, "", "", "", Provider.NONE),
            new Row("custom wins over the account", true, true, false, "", "", true,
                "https://translate.example/api", "", false, "", "", "", Provider.CUSTOM),
            new Row("NiuTrans wins over custom", true, false, true, "app", "key", true,
                "https://translate.example/api", "", false, "", "", "", Provider.NIUTRANS),
            new Row("custom wins over Tencent", true, false, false, "", "", true,
                "https://translate.example/api", "", true, "AKIDsynthetic", "synthetic", "", Provider.CUSTOM),
            new Row("usable Tencent wins over the account", true, true, false, "", "", false, "", "",
                true, "AKIDsynthetic", "synthetic", "", Provider.TENCENT),
            new Row("disabled Tencent leaves the account", true, true, false, "", "", false, "", "",
                false, "AKIDsynthetic", "synthetic", "", Provider.ACCOUNT),
            new Row("Tencent placeholder is no credential", true, true, false, "", "", false, "", "",
                true, "<SecretId>", "synthetic", "", Provider.ACCOUNT),
            new Row("Tencent FAKESECRET is no credential", true, false, false, "", "", false, "", "",
                true, "AKIDsynthetic", "FAKESECRET_key", "", Provider.NONE),
            new Row("NiuTrans whitespace is no credential", true, false, true, " \n", "key", false, "", "",
                false, "", "", "", Provider.NONE),
            new Row("translations off sends nothing to NiuTrans", false, false, true, "app", "key", false, "", "",
                false, "", "", "", Provider.NONE),
            new Row("translations off sends nothing to custom", false, false, false, "", "", true,
                "https://translate.example/api", "", false, "", "", "", Provider.NONE),
            new Row("translations off sends nothing to Tencent", false, false, false, "", "", false, "", "",
                true, "AKIDsynthetic", "synthetic", "", Provider.NONE),
            new Row("translations off sends nothing to the account", false, true, false, "", "", false, "", "",
                false, "", "", "", Provider.NONE),
        };
        for (Row row : rows) {
            Provider actual = row.route().provider();
            check(actual == row.expected(), row.name() + ": expected " + row.expected() + ", got " + actual);
        }

        check(!CandidateTranslationPolicy.usableCredential("<key>")
                && !CandidateTranslationPolicy.usableCredential("FAKESECRET_x")
                && !CandidateTranslationPolicy.usableCredential(" \t\r\n")
                && !CandidateTranslationPolicy.usableCredential(null)
                && CandidateTranslationPolicy.usableCredential(" key "),
            "placeholders, FAKESECRET_ samples and blank values are not credentials");

        CandidateTranslationRoute tencent = CandidateTranslationPolicy.route(true, false, false, "", "",
            false, "", "", true, " AKIDsynthetic ", "secret-value", "");
        check("AKIDsynthetic".equals(tencent.secretId()), "credentials are trimmed");
        check(CandidateTranslationRoute.DEFAULT_TENCENT_REGION.equals(tencent.region()),
            "an empty Tencent region takes the shared default");
        check(!tencent.toString().contains("secret-value") && !tencent.toString().contains("AKID"),
            "toString never carries credentials");
        check(!tencent.scope().contains("secret-value"), "the cache scope never carries credentials");

        CandidateTranslationRoute custom = CandidateTranslationPolicy.route(true, false, false, "", "",
            true, " https://translate.example/api ", "key-one", false, "", "", "");
        CandidateTranslationRoute rotated = CandidateTranslationPolicy.route(true, false, false, "", "",
            true, "https://translate.example/api", "key-two", false, "", "", "");
        CandidateTranslationRoute same = CandidateTranslationPolicy.route(true, false, false, "", "",
            true, "https://translate.example/api", "key-one", false, "", "", "");
        check("https://translate.example/api".equals(custom.endpoint()), "the endpoint is trimmed");
        check(!custom.scope().equals(rotated.scope()) && !custom.equals(rotated),
            "changing the key changes the route and its cache scope");
        check(custom.scope().equals(same.scope()) && custom.equals(same) && custom.hashCode() == same.hashCode(),
            "the same settings give the same route");
        check(custom.userService() && !CandidateTranslationRoute.ACCOUNT.userService()
                && !CandidateTranslationRoute.NONE.userService(),
            "only direct providers are user services");
        CandidateTranslationRoute ai = CandidateTranslationPolicy.route(true, false, false, "", "",
            false, "", "", false, "", "", "", true, "openai",
            "https://api.openai.com/v1/chat/completions", "synthetic-model", "synthetic-token");
        check(ai.provider() == Provider.AI && ai.userService()
                && "openai".equals(ai.aiProvider()) && "synthetic-model".equals(ai.aiModel()),
            "valid AI settings select the shared AI translation route");
        CandidateTranslationRoute aiFallback = CandidateTranslationPolicy.route(true, false, false, "", "",
            false, "", "", false, "", "", "", true, "openai",
            "https://api.openai.com/v1/chat/completions", "synthetic-model", "");
        check(aiFallback.provider() == Provider.NONE, "AI without a token does not send text");
        // 「AI 辅助」要显式选择（`translation_ai`），与共享层 `selected_translation_services` 相同：AI 联想开没开不参与，别的服务被选中时轮不到它，选中的服务凭据不全时也不改用它。
        String endpoint = "https://api.openai.com/v1/chat/completions";
        check(CandidateTranslationPolicy.route(true, false, false, "", "", false, "", "", false, "", "", "",
                false, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.NONE,
            "a usable AI configuration is not used for translation unless AI-assisted translation is chosen");
        check(CandidateTranslationPolicy.route(false, false, false, "", "", false, "", "", false, "", "", "",
                true, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.NONE,
            "translations off sends nothing to AI");
        check(CandidateTranslationPolicy.route(true, false, false, "", "", false, "", "", true, "", "", "",
                true, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.NONE,
            "an enabled Tencent without credentials stays selected ahead of AI, as in the shared layer");
        check(CandidateTranslationPolicy.route(true, false, true, "app", "", false, "", "", false, "", "", "",
                true, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.NONE,
            "NiuTrans without a key never falls back to AI");
        check(CandidateTranslationPolicy.route(true, false, false, "", "", true, " ", "", false, "", "", "",
                true, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.NONE,
            "custom without an endpoint never falls back to AI");
        check(CandidateTranslationPolicy.route(true, true, false, "", "", false, "", "", false, "", "", "",
                true, "openai", endpoint, "synthetic-model", "synthetic-token").provider() == Provider.ACCOUNT,
            "an explicit account choice wins over AI, as in the shared layer");
        System.out.println("Android candidate translation route mirrors the shared provider selection");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
