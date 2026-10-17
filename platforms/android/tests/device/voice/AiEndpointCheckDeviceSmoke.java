package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.Method;

/** 经真实的 libmsime_android.so 核对 AI 接口地址的检查：AiEndpointPolicy 把地址交给 Rust 的 msime_client_ai_endpoint_check，结论和来源键原样读回。JVM 冒烟加载不了 JNI，只能在这里验。 */
public final class AiEndpointCheckDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_AI_ENDPOINT_CHECK_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_AI_ENDPOINT_CHECK_FAILED: " + error.getClass().getSimpleName()
                + ": " + error.getMessage() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void runCases() throws ReflectiveOperationException {
        Class<?> policy = getTargetContext().getClassLoader().loadClass("app.msime.android.AiEndpointPolicy");
        Method check = policy.getMethod("check", String.class);
        Method origin = policy.getMethod("credentialOrigin", String.class);
        // 取自 shared/contracts/ai-endpoint/cases.json：本机和局域网的 http、公网 https、公网 http、带片段的地址。
        expect(check, origin, "HTTP://LOCALHOST:1234/v1", "ALLOWED", "http://localhost:1234");
        expect(check, origin, "http://192.168.1.20:1234/v1/chat/completions", "ALLOWED", "http://192.168.1.20:1234");
        expect(check, origin, "https://API.Example.com/v1", "ALLOWED", "https://api.example.com:443");
        expect(check, origin, "http://api.example.com/v1", "CLEARTEXT_PUBLIC", null);
        expect(check, origin, "https://api.example.com/v1#", "INVALID", null);
        // 原来的 Java 副本与 Rust 结论不同的两种写法：带前导零的 IPv4 原来按公网明文拒绝，Rust 按不合规拒绝；国际化域名原来解析不出主机，Rust 按 punycode 放行。
        expect(check, origin, "http://010.0.0.1/v1", "INVALID", null);
        expect(check, origin, "https://例子.测试/v1", "ALLOWED", "https://xn--fsqu00a.xn--0zwm56d:443");
        // 首尾空格交给 Rust 去掉，制表符仍是控制字符。
        expect(check, origin, " https://api.example.com/v1 ", "ALLOWED", "https://api.example.com:443");
        expect(check, origin, "\thttps://api.example.com/v1", "INVALID", null);
        expect(check, origin, "", "INVALID", null);
        // 发请求用的地址经 Android 自己的 java.net.URI 构造：国际化域名的主机换成 punycode；Rust 放行而 URI 不收的写法（路径里有空格）整体不合规，设置页也不收。
        Method uri = policy.getMethod("uri", String.class);
        String target = String.valueOf(uri.invoke(null, "https://例子.测试/v1"));
        if (!"https://xn--fsqu00a.xn--0zwm56d/v1".equals(target))
            throw new AssertionError("https://例子.测试/v1: expected a punycode request target, got " + target);
        expect(check, origin, "https://api.example.com/v1 chat", "INVALID", null);
    }

    private static void expect(Method check, Method origin, String endpoint, String result, String key)
            throws ReflectiveOperationException {
        String actual = String.valueOf(check.invoke(null, endpoint));
        if (!result.equals(actual)) throw new AssertionError(endpoint + ": expected " + result + ", got " + actual);
        Object actualKey = origin.invoke(null, endpoint);
        if (key == null ? actualKey != null : !key.equals(actualKey))
            throw new AssertionError(endpoint + ": expected origin " + key + ", got " + actualKey);
    }
}
