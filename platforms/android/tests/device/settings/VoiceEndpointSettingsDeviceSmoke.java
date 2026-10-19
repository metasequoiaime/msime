package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.Method;
import org.json.JSONObject;

/** 使用合成地址验证设置页经真实 JNI 复用语音运行时校验；不保存偏好、不联网。 */
public final class VoiceEndpointSettingsDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            result.putString("stream", "MSIME_VOICE_ENDPOINT_FAILED: " + error.getClass().getSimpleName() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private void runCases() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        Class<?> policy = loader.loadClass("app.msime.android.VoiceProviderPolicy");
        Method check = policy.getMethod("endpointProblem", String.class, String.class);
        for (String invalid : new String[] {
                "https://example.invalid:bad/v1", "https://[broken/v1", "https://bad host/v1",
                "https:///v1", "https://user:synthetic@example.invalid/v1", "https://example.invalid/v1#fragment",
                "https://example.invalid/path@synthetic", "HTTPS://example.invalid/v1",
                "http://example.invalid/v1", "wss://example.invalid/v1", "https://example.invalid/\u0000",
                "https://example.invalid/" + "合".repeat(800)}) {
            require(check.invoke(null, "openai", invalid) != null, "上传服务拒绝无效地址");
        }
        for (String valid : new String[] {"", "  ", "https://example.invalid/v1", " https://example.invalid/v1 ",
                "https://例子.invalid/合成🍃"}) {
            require(check.invoke(null, "openai", valid) == null, "设置页接受运行时支持的地址或空值");
        }
        require(check.invoke(null, "openai", null) == null, "缺省地址使用默认值");
        require(check.invoke(null, "doubao", "wss://example.invalid/stream") == null, "豆包使用安全流式地址");
        require(check.invoke(null, "doubao", "https://example.invalid/stream") != null, "豆包不接受上传式协议");
        require(check.invoke(null, "openai", "\ud800") != null, "无效 UTF-16 不能在传输中变成可接受的替代字符");

        Method parse = policy.getDeclaredMethod("endpointIssue", String.class);
        parse.setAccessible(true);
        require(parse.invoke(null, "{\"ok\":true,\"value\":{\"accepted\":true,\"issue\":null}}") == null,
            "明确的成功补丁允许保存");
        String expected = "合成错误提示";
        require(expected.equals(parse.invoke(null, new JSONObject().put("ok", true).put("value",
            new JSONObject().put("accepted", false).put("issue", expected)).toString())), "校验提示来自共享层");
        for (String broken : new String[] {"", "{}", "{\"ok\":\"true\",\"value\":{\"accepted\":true,\"issue\":null}}",
                "{\"ok\":true,\"value\":{\"accepted\":\"true\",\"issue\":null}}",
                "{\"ok\":true,\"value\":{\"accepted\":true}}",
                "{\"ok\":true,\"value\":{\"accepted\":true,\"issue\":\"synthetic\"}}",
                "{\"ok\":true,\"value\":{\"accepted\":false,\"issue\":null}}"}) {
            require(parse.invoke(null, broken) != null, "损坏或矛盾响应不能放行保存");
        }
        Class<?> bridge = loader.loadClass("app.msime.android.NativeClient");
        Method bounded = bridge.getMethod("voiceEndpointIssue", String.class, boolean.class);
        for (String oversized : new String[] {"x".repeat(65_537), "合".repeat(30_000)}) {
            try {
                bounded.invoke(null, oversized, false);
                throw new AssertionError("Java 必须在 JNI 前限制 UTF-8 大小");
            } catch (java.lang.reflect.InvocationTargetException error) {
                require(error.getCause() instanceof IllegalArgumentException, "超限返回固定参数错误");
            }
        }
        Method raw = bridge.getDeclaredMethod("voiceEndpointIssueRaw", byte[].class, boolean.class);
        raw.setAccessible(true);
        for (byte[] payload : new byte[][] {null, new byte[65_537]}) {
            byte[] response = (byte[]) raw.invoke(null, payload, false);
            require(Boolean.FALSE.equals(new JSONObject(new String(response, java.nio.charset.StandardCharsets.UTF_8)).opt("ok")),
                "JNI 拒绝空引用或超限请求");
        }
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
