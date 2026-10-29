package app.msime.android;

/**
 * 「翻译服务」设置页的纯逻辑：页面显示哪一项被选中、每个字段能不能写进偏好、连接测试用哪个服务名。不依赖 Android 和 org.json，JVM 冒烟直接测它。
 *
 * <p>字段规则逐条照共享偏好的 `Preferences::validate`（`crates/client-core/src/preferences.rs`）：不合规的值整份偏好都存不进去，所以在输入框里当场指出是哪个字段、为什么不行，而不是等保存时只说「保存失败」。
 */
public final class TranslationServiceSettingsPolicy {
    /** 偏好里凭据和密钥的字节上限。 */
    public static final int MAX_CREDENTIAL_BYTES = 4096;
    /** 腾讯云地域的字节上限。 */
    public static final int MAX_REGION_BYTES = 64;

    /** 页面上的六个选项，顺序就是单选列表的顺序。 */
    public enum Choice {
        NONE("不使用", null),
        ACCOUNT("水杉账号", null),
        NIUTRANS("小牛翻译", "translation.niutrans"),
        TENCENT("腾讯云机器翻译", "translation.tencent"),
        CUSTOM("自定义（DeepLX 或 DeepL API）", "translation.custom"),
        /** 复用「AI 润色与回复」页的服务商、接口地址、令牌和模型，写 `translation_ai`；配置在那一页改，这里没有自己的凭据和连接测试。 */
        AI("AI 辅助", null);

        private final String title;
        private final String testService;

        Choice(String title, String testService) {
            this.title = title;
            this.testService = testService;
        }

        public String title() { return title; }

        /** `msime_client_test_api_credential` 的服务名；不使用和水杉账号没有可测的东西，为 null。 */
        public String testService() { return testService; }

        /** 是不是用户自己的服务，要填凭据。 */
        public boolean userService() { return testService != null; }
    }

    private TranslationServiceSettingsPolicy() {}

    /**
     * 页面显示为选中的那一项，与 iOS 的 `TranslationProviderPreference.selected` 相同：先看小牛、自定义的启用，再看腾讯，再看水杉账号，最后看 AI 辅助（`translation_ai`）。小牛和自定义只要启用就算选中（凭据不全时页面提示「凭据不完整」）；腾讯的 `enabled` 默认就开着，所以只有两项凭据都可用才算用户选了它，否则新装的用户会看到一个自己没选过的腾讯。腾讯启用而没有凭据时，共享层的选路照样先选中它、轮不到 AI（见 {@link CandidateTranslationPolicy#route}），键盘什么都不发，所以这时 AI 辅助也显示成「不使用」。AI 配置不全时 AI 辅助仍显示为选中，页面提示去补配置。
     */
    public static Choice selected(boolean translationAccount, boolean niutransEnabled, boolean customEnabled,
            boolean tencentEnabled, String tencentSecretId, String tencentSecretKey, boolean translationAi) {
        if (niutransEnabled) return Choice.NIUTRANS;
        if (customEnabled) return Choice.CUSTOM;
        if (tencentEnabled && CandidateTranslationPolicy.usableCredential(tencentSecretId)
                && CandidateTranslationPolicy.usableCredential(tencentSecretKey)) return Choice.TENCENT;
        if (translationAccount) return Choice.ACCOUNT;
        return translationAi && !tencentEnabled ? Choice.AI : Choice.NONE;
    }

    /** 小牛的 App ID 或 API Key：可以留空；填了就不能是 `<...>` 占位符或 `FAKESECRET_` 样例。返回给用户看的原因，可以保存时为 null。 */
    public static String niutransProblem(String value) {
        String bounded = boundedProblem(value);
        if (bounded != null || value.isEmpty()) return bounded;
        return CandidateTranslationPolicy.usableCredential(value) ? null : "请填写真实的凭据，不要保留示例占位符";
    }

    /** 腾讯的 SecretId：可以留空；只能是英文字母、数字、- 和 _。 */
    public static String secretIdProblem(String value) {
        if (value == null || TextPolicy.utf8Length(value) > MAX_CREDENTIAL_BYTES) return "SecretId 太长";
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            boolean allowed = (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z')
                || (character >= '0' && character <= '9') || character == '-' || character == '_';
            if (!allowed) return "SecretId 只能包含英文字母、数字、- 和 _";
        }
        return null;
    }

    /** 腾讯的 SecretKey 或自定义服务的 API Key：可以留空，不能有控制字符，不超过 4096 字节。 */
    public static String secretProblem(String value) {
        return boundedProblem(value);
    }

    /** 腾讯的地域：可以留空（留空时写入默认地域）；只能是英文字母、数字和 -，最多 64 个字符。 */
    public static String regionProblem(String value) {
        if (value == null || value.length() > MAX_REGION_BYTES) return "地域最多 64 个字符";
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            boolean allowed = (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z')
                || (character >= '0' && character <= '9') || character == '-';
            if (!allowed) return "地域只能包含英文字母、数字和 -，例如 ap-guangzhou";
        }
        return null;
    }

    /**
     * 自定义服务的接口地址，以共享层为准：{@code accepted} 是 `msime_client_translation_endpoint_issue` 回答里的 `accepted`，与保存偏好时的校验是同一条 `is_supported_endpoint`，{@code issue} 是它给的提示。共享库不可用时 {@code accepted} 为 null，退回 {@link #endpointProblem(String)} 的本地规则。共享层不接受但没给提示时（例如 `https:///v1`），用本地规则的说法。
     */
    public static String endpointProblem(String value, Boolean accepted, String issue) {
        if (accepted == null) return endpointProblem(value);
        if (accepted) return null;
        if (issue != null && !issue.isEmpty()) return issue;
        String local = endpointProblem(value);
        return local != null ? local : "请填写以 https:// 开头的完整接口地址，不能带用户名、密码或 # 片段";
    }

    /** 共享库不可用时的本地规则：可以留空；填了就要是 https，或指向本机回环的 http，规则同 {@link TranslationHttpTransport#allowedURL}。 */
    public static String endpointProblem(String value) {
        if (value == null || value.isEmpty()) return null;
        if (TranslationHttpTransport.allowedURL(value)) return null;
        String lower = TextPolicy.lowercase(value);
        if (lower.startsWith("http://")) return "远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址（localhost、127.0.0.1、[::1]）";
        return "请填写以 https:// 开头的完整接口地址，不能带用户名、密码或 # 片段";
    }

    private static String boundedProblem(String value) {
        if (value == null) return "内容无效";
        if (TextPolicy.utf8Length(value) > MAX_CREDENTIAL_BYTES) return "内容太长，最多 4096 字节";
        if (TextPolicy.hasControl(value) || !TextPolicy.validUnicode(value)) return "不能包含控制字符";
        return null;
    }
}
