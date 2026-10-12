import app.msime.android.TranslationHttpTransport;

/** 用户翻译服务的地址规则与共享层 `translation::is_secure_endpoint` 一致，超时与回复上限按描述器夹取。 */
public final class TranslationHttpTransportPolicySmoke {
    public static void main(String[] args) {
        String[] allowed = {
            "https://api-free.deepl.com/v2/translate",
            "https://translate.example:8443/api",
            "http://127.0.0.1:1188/translate",
            "http://localhost:1188/translate",
            "http://LOCALHOST/translate",
            "http://[::1]:1188/translate",
            // 共享层按 WHATWG 规则放行的 https 地址：下划线主机名、国际化域名、查询串里的 `|`。
            "https://my_host.example.com/translate",
            "https://例子.example/translate",
            "https://deeplx.example.com/translate?token=a|b",
        };
        for (String url : allowed) check(TranslationHttpTransport.allowedURL(url), "allowed: " + url);
        String[] rejected = {
            null,
            "",
            "http://192.168.1.20:1188/translate",
            "http://10.0.2.2:1188/translate",
            "http://translate.local/translate",
            "http://translate.example/api",
            "https://user:secret@translate.example/api",
            "https://user@translate.example/api",
            "https://translate.example/api#fragment",
            "https://translate.example/api#",
            "ftp://translate.example/api",
            "https:///v1",
            "translate.example/api",
            "https://translate.example/a\nb",
            "https://translate.example:70000/api",
            "http://my_host.example.com/translate",
        };
        for (String url : rejected) check(!TranslationHttpTransport.allowedURL(url), "rejected: " + url);

        check(TranslationHttpTransport.timeoutMillis(null) == TranslationHttpTransport.DEFAULT_TIMEOUT_MILLIS,
            "a missing timeout uses the shared descriptor's default");
        check(TranslationHttpTransport.timeoutMillis(2_500) == 2_500, "the shared timeout is kept");
        check(TranslationHttpTransport.timeoutMillis(1) == TranslationHttpTransport.MIN_TIMEOUT_MILLIS,
            "a tiny timeout is raised to one second");
        check(TranslationHttpTransport.timeoutMillis(600_000) == TranslationHttpTransport.MAX_TIMEOUT_MILLIS,
            "a long timeout is capped at ten seconds");
        check(TranslationHttpTransport.timeoutMillis(2.5) == TranslationHttpTransport.DEFAULT_TIMEOUT_MILLIS
                && TranslationHttpTransport.timeoutMillis("2500") == TranslationHttpTransport.DEFAULT_TIMEOUT_MILLIS,
            "a timeout that is not an integer is ignored");
        check(TranslationHttpTransport.responseLimit(null) == TranslationHttpTransport.MAX_RESPONSE_BYTES,
            "a missing response limit is one MiB");
        check(TranslationHttpTransport.responseLimit(4096) == 4096, "a smaller response limit is kept");
        check(TranslationHttpTransport.responseLimit(64 * 1024 * 1024) == TranslationHttpTransport.MAX_RESPONSE_BYTES,
            "a larger response limit is capped at one MiB");
        check(TranslationHttpTransport.responseLimit(0) == 0 && TranslationHttpTransport.responseLimit(-1) == 0
                && TranslationHttpTransport.responseLimit("1024") == 0,
            "an unusable response limit refuses the descriptor");
        System.out.println("Android translation transport: endpoint rule, timeout and response bounds passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
