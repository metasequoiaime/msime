package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.io.File;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.util.HashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 经安装产品里真实的 libmsime_android.so 核对 #8201 新加的账号与云端 JNI 入口（迁移计划 5a，还没有调用方）：NativeClient.SessionStore 注册给 Rust 以后，Rust 经它读出一份合成的 Native 布局会话，status 和 access_token 认得它；存储方法抛出的异常在 JNI 一侧变成 account_storage，不抛给调用方；注销以后同一个 id 不再可用。其余入口各发一个空请求，按各自的「请求不合法」码确认 JNI 名字连到了对的 Rust 函数。不联网，合成会话没有过期，不会去刷新。
 *
 * <p>没 attach 的线程上的回调这里造不出来（Rust 自己的线程只在联网刷新时回调存储），由 check-host.sh 在桌面 JVM 上驱动 native/JniCallbacks.cpp 验证。
 */
public final class SessionStoreDeviceSmoke extends Instrumentation {
    private static final String USER = "synthetic-session-store-user";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            runCases();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED\n");
            finish(Activity.RESULT_OK, result);
        } catch (Throwable error) {
            Throwable cause = error.getCause() != null ? error.getCause() : error;
            result.putString("stream", "MSIME_SESSION_STORE_FAILED: " + cause.getClass().getSimpleName()
                + ": " + cause.getMessage() + "\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private Class<?> client;
    private Class<?> storeType;

    private void runCases() throws Exception {
        ClassLoader loader = getTargetContext().getClassLoader();
        client = loader.loadClass("app.msime.android.NativeClient");
        storeType = loader.loadClass("app.msime.android.NativeClient$SessionStore");

        // client_agent 由 Rust 拼装，参数与 BackendAccount 原来的 loginUserAgent 相同。
        JSONObject agentRequest = new JSONObject().put("platform", "android").put("version", "1.2.3")
            .put("device", "Pixel 6").put("system", "Android 9").put("edition", "full");
        String agent = value(call("clientAgentRaw", agentRequest)).getString("value");
        check("msime-android/1.2.3 (Pixel 6; Android 9; edition=full)".equals(agent), "client agent: " + agent);

        MemoryStore memory = new MemoryStore(false);
        memory.items.put(1, sessionBytes());
        long id = register(memory, agent.getBytes(StandardCharsets.UTF_8));
        JSONObject status = value(call("accountRaw", new JSONObject().put("session_store", id)
            .put("action", new JSONObject().put("operation", "status"))));
        check(USER.equals(status.getJSONObject("value").getJSONObject("user").getString("id")),
            "status did not read the session through the store: " + status);
        check(memory.loads > 0 && memory.lastThread == Thread.currentThread(),
            "the store was not read on the calling thread");
        JSONObject token = value(call("accountAccessTokenRaw", new JSONObject().put("session_store", id)))
            .getJSONObject("value");
        check(hex('a').equals(token.getString("access_token")) && USER.equals(token.getString("user_id"))
            && !token.getBoolean("anonymous"), "access token: " + token);

        // 存储方法抛出的异常在 JNI 一侧清掉，Rust 把它当读取失败。
        MemoryStore throwing = new MemoryStore(true);
        long broken = register(throwing, null);
        expectError(call("accountRaw", new JSONObject().put("session_store", broken)
            .put("action", new JSONObject().put("operation", "status"))), "account_storage");
        check(throwing.loads > 0, "the throwing store was never asked");
        check(value(callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, broken)).getBoolean("value"),
            "unregister of the throwing store");

        // 含 NUL 的 client_agent 在 JNI 里就拒绝，免得 Rust 读到 NUL 停下、把前半段当成合法的。
        expectError(callRaw("sessionStoreRegisterRaw", new Class<?>[] {storeType, byte[].class},
            proxy(new MemoryStore(false)), (agent + "\0garbage").getBytes(StandardCharsets.UTF_8)), "invalid_session_store");
        expectError(callRaw("sessionStoreRegisterRaw", new Class<?>[] {storeType, byte[].class}, null, null),
            "invalid_session_store");

        JSONObject unregistered = value(callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, id));
        check(unregistered.getBoolean("value"), "unregister of a registered store");
        check(!value(callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, id)).getBoolean("value"),
            "a second unregister");
        expectError(call("accountRaw", new JSONObject().put("session_store", id)
            .put("action", new JSONObject().put("operation", "status"))), "account_invalid");

        // 旧版 Java 快照队列的就地转换：目录里没有 state.bin 时什么也不改。
        File queue = new File(getTargetContext().getCacheDir(), "synthetic-session-store-queue");
        if (!queue.isDirectory() && !queue.mkdirs()) throw new AssertionError("queue directory");
        JSONObject imported = value(call("snapshotQueueRaw", new JSONObject().put("operation", "import_android_v1")
            .put("directory", queue.getAbsolutePath())));
        check(!imported.getJSONObject("value").getBoolean("imported"), "an empty queue was imported: " + imported);

        // 释义的 sans-IO 一半：不联网，只拼请求。
        JSONObject gloss = value(call("accountGlossRaw", new JSONObject().put("operation", "request")
            .put("texts", new JSONArray().put("你好")).put("source_lang", "zh").put("target_lang", "en")))
            .getJSONObject("value");
        check("/v1/translate".equals(gloss.getString("path")), "gloss request: " + gloss);

        // 其余入口：空请求各自回自己的「请求不合法」码，说明 JNI 名字连到了对的 Rust 函数。
        Map<String, String> invalid = new HashMap<>();
        invalid.put("accountCloudClipboardRaw", "invalid_cloud_clipboard");
        invalid.put("accountChatCancelRaw", "account_invalid");
        invalid.put("communityRaw", "community_invalid");
        invalid.put("aiSkinCancelRaw", "ai_skin_invalid");
        invalid.put("aiSkinJobRaw", "ai_skin_invalid");
        invalid.put("cloudDictionaryRaw", "invalid_cloud_dictionary");
        invalid.put("snapshotQueueRaw", "snapshot_invalid");
        invalid.put("cloudSyncRaw", "cloud_sync_invalid");
        for (Map.Entry<String, String> entry : invalid.entrySet()) {
            expectError(call(entry.getKey(), new JSONObject()), entry.getValue());
        }
        Class<?> delta = client.getClassLoader().loadClass("app.msime.android.NativeClient$ChatDelta");
        Class<?> progress = client.getClassLoader().loadClass("app.msime.android.NativeClient$AiSkinProgress");
        expectError(callRaw("accountChatStreamRaw", new Class<?>[] {byte[].class, delta}, bytes(new JSONObject()), null),
            "account_invalid");
        expectError(callRaw("aiSkinGenerateRaw", new Class<?>[] {byte[].class, progress}, bytes(new JSONObject()), null),
            "ai_skin_invalid");
    }

    private long register(MemoryStore store, byte[] agent) throws Exception {
        JSONObject registered = value(callRaw("sessionStoreRegisterRaw", new Class<?>[] {storeType, byte[].class},
            proxy(store), agent));
        long id = registered.getJSONObject("value").getLong("session_store");
        check(id > 0, "session store id: " + registered);
        return id;
    }

    private Object proxy(MemoryStore store) {
        return Proxy.newProxyInstance(storeType.getClassLoader(), new Class<?>[] {storeType}, store);
    }

    private byte[] call(String name, JSONObject request) throws Exception {
        return callRaw(name, new Class<?>[] {byte[].class}, bytes(request));
    }

    private byte[] callRaw(String name, Class<?>[] types, Object... arguments) throws Exception {
        Method method = client.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return (byte[]) method.invoke(null, arguments);
    }

    private static byte[] bytes(JSONObject request) {
        return request.toString().getBytes(StandardCharsets.UTF_8);
    }

    private static String text(byte[] response) {
        if (response == null) throw new AssertionError("null response");
        return new String(response, StandardCharsets.UTF_8);
    }

    private static JSONObject value(byte[] response) throws Exception {
        JSONObject envelope = new JSONObject(text(response));
        if (!envelope.optBoolean("ok", false)) throw new AssertionError("expected success: " + envelope);
        return envelope;
    }

    private static void expectError(byte[] response, String code) throws Exception {
        JSONObject envelope = new JSONObject(text(response));
        if (envelope.optBoolean("ok", true) || !code.equals(envelope.optString("error")))
            throw new AssertionError("expected " + code + ", got " + envelope);
    }

    /** 一份未过期的合成会话，Native 布局（SavedAccountSession），与 Java 原来写出的 v2 会话同形。 */
    private static byte[] sessionBytes() throws Exception {
        JSONObject user = new JSONObject().put("id", USER).put("display_name", "合成用户")
            .put("created_at", "2026-01-01T00:00:00Z");
        JSONObject tokens = new JSONObject().put("access_token", hex('a')).put("refresh_token", hex('d'))
            .put("token_type", "Bearer").put("expires_in", 3600).put("user", user);
        return new JSONObject().put("tokens", tokens)
            .put("expires_at_unix_ms", System.currentTimeMillis() + 3_600_000L)
            .put("session_id", "00000000-0000-0000-0000-000000000009")
            .toString().getBytes(StandardCharsets.UTF_8);
    }

    private static String hex(char digit) {
        StringBuilder text = new StringBuilder(64);
        for (int index = 0; index < 64; index++) text.append(digit);
        return text.toString();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** 内存里的会话存储，经 java.lang.reflect.Proxy 实现产品里的 NativeClient.SessionStore。 */
    private static final class MemoryStore implements InvocationHandler {
        final Map<Integer, byte[]> items = new HashMap<>();
        final boolean failing;
        volatile int loads;
        volatile Thread lastThread;

        MemoryStore(boolean failing) { this.failing = failing; }

        @Override public synchronized Object invoke(Object proxy, Method method, Object[] arguments) {
            switch (method.getName()) {
                case "load":
                    loads++;
                    lastThread = Thread.currentThread();
                    if (failing) throw new IllegalStateException("合成的存储失败");
                    return items.get((Integer) arguments[0]);
                case "save":
                    if (failing) throw new IllegalStateException("合成的存储失败");
                    items.put((Integer) arguments[0], ((byte[]) arguments[1]).clone());
                    return 0;
                case "remove":
                    if (failing) throw new IllegalStateException("合成的存储失败");
                    items.remove((Integer) arguments[0]);
                    return 0;
                case "hashCode":
                    return System.identityHashCode(proxy);
                case "equals":
                    return proxy == arguments[0];
                case "toString":
                    return "MemoryStore";
                default:
                    throw new UnsupportedOperationException(method.getName());
            }
        }
    }
}
