package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.lang.reflect.Field;
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
        Method descriptor = bridge.getMethod("voiceWebSocketEndpoint", String.class);
        JSONObject descriptorRoot = new JSONObject((String) descriptor.invoke(null,
            "wss://例子.invalid:8443/合成🍃/%2F?lang=zh+cn&empty="));
        JSONObject descriptorValue = descriptorRoot.getJSONObject("value");
        require("xn--fsqu00a.invalid".equals(descriptorValue.getString("socket_host")),
            "共享描述符使用规范化域名");
        require(descriptorValue.getInt("port") == 8443,
            "共享描述符保留非默认端口");
        require("xn--fsqu00a.invalid:8443".equals(descriptorValue.getString("host_header")),
            "共享描述符保留 Host 端口");
        require("/%E5%90%88%E6%88%90%F0%9F%8D%83/%2F?lang=zh+cn&empty="
                .equals(descriptorValue.getString("request_target")),
            "共享描述符保留路径与查询编码");
        JSONObject ipv6 = new JSONObject((String) descriptor.invoke(null, "wss://[::1]/stream?"))
            .getJSONObject("value");
        require("::1".equals(ipv6.getString("socket_host"))
                && "[::1]".equals(ipv6.getString("host_header"))
                && "/stream?".equals(ipv6.getString("request_target")),
            "共享描述符分别格式化 IPv6 socket 和 Host");
        Class<?> endpointType = loader.loadClass("app.msime.android.DoubaoRecognizer$Endpoint");
        Method transportParse = endpointType.getDeclaredMethod("parse", String.class);
        transportParse.setAccessible(true);
        Class<?> frames = loader.loadClass("app.msime.android.WebSocketFrames");
        Method handshake = frames.getDeclaredMethod("handshakeRequest", String.class, String.class,
            String.class, String[].class);
        handshake.setAccessible(true);
        for (String address : new String[] {"wss://例子.invalid:8443/合成🍃/%2F?lang=zh+cn&empty=",
                "wss://[::1]/stream?", "wss://example.invalid:0/%zz?value=%"}) {
            JSONObject expectedDescriptor = new JSONObject((String) descriptor.invoke(null, address))
                .getJSONObject("value");
            Object parsed = transportParse.invoke(null, address);
            require(parsed != null, "实际传输入口接受共享描述符");
            for (String[] names : new String[][] {{"socketHost", "socket_host"},
                    {"port", "port"}, {"hostHeader", "host_header"}, {"requestTarget", "request_target"}}) {
                Field field = endpointType.getDeclaredField(names[0]);
                field.setAccessible(true);
                require(expectedDescriptor.get(names[1]).equals(field.get(parsed)),
                    "实际传输字段与共享描述符一致");
            }
            String request = (String) handshake.invoke(null, expectedDescriptor.getString("host_header"),
                expectedDescriptor.getString("request_target"), "c3ludGhldGlj", new String[0]);
            require(request.startsWith("GET " + expectedDescriptor.getString("request_target") + " HTTP/1.1\r\n")
                    && request.contains("\r\nHost: " + expectedDescriptor.getString("host_header") + "\r\n"),
                "握手保留实际路径与 Host");
        }
        for (String invalid : new String[] {"wss://example.invalid:65536/stream", "wss://example.invalid/stream\nX: y"}) {
            JSONObject rejected = new JSONObject((String) descriptor.invoke(null, invalid));
            require(!rejected.optBoolean("ok", false), "共享描述符拒绝无效端点");
            require(transportParse.invoke(null, invalid) == null, "实际传输入口拒绝无效端点");
        }
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
