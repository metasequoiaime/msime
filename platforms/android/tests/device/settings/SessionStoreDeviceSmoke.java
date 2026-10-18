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
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 经安装产品里真实的 libmsime_android.so 核对 #8201 新加的账号与云端 JNI 入口：NativeClient.SessionStore 注册给 Rust 以后，Rust 经它读出一份合成的 Native 布局会话，status 和 access_token 认得它；存储方法抛出的异常在 JNI 一侧变成 account_storage，不抛给调用方；注销以后同一个 id 不再可用。其余入口各发一个空请求，按各自的「请求不合法」码确认 JNI 名字连到了对的 Rust 函数。不联网，合成会话没有过期，不会去刷新。
 *
 * <p>迁移计划 5b 起产品经 AccountStore 注册：它的存储适配器把 Java 原来写的 v2 会话（AndroidKeyStore 加密的 SharedPreferences）原样交给 Rust，匿名身份「不存在才写」，云剪贴板密钥串是 CloudClipboardKeystore 的那个文件，删除不等它的锁；AccountStore 只在主进程里注册一次，`:ime` 进程名和第二次注册都在碰 Rust 之前就拒绝。
 *
 * <p>没 attach 的线程上的回调这里造不出来（Rust 自己的线程只在联网刷新时回调存储），由 check-host.sh 在桌面 JVM 上驱动 native/JniCallbacks.cpp 验证。
 */
public final class SessionStoreDeviceSmoke extends Instrumentation {
    private static final String USER = "synthetic-session-store-user";
    /** 冒烟里占着密钥串锁的最长时间；同步删除的 remove(2) 至少要等这么久才返回。 */
    private static final long LOCK_HOLD_SECONDS = 5;

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

        accountStoreAdapter();
        accountStoreKeystore();
        accountStoreGuards();
        voiceContributionPaths();

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

    /**
     * 产品的存储适配器（AccountStore 的 Items）原样交出 Java 原来写的 v2 会话：用 AndroidAccountSessionStorage 加密写进 `msime_account_session_v2`，再把适配器另外注册一次，Rust 的 status 读得出这个账号。匿名身份的 save 是「不存在才写」。用到的项最后都删掉。
     */
    private void accountStoreAdapter() throws Exception {
        android.content.Context target = getTargetContext();
        ClassLoader loader = target.getClassLoader();
        Class<?> storage = loader.loadClass("app.msime.android.AndroidAccountSessionStorage");
        Object v2 = storage.getConstructor(android.content.Context.class, String.class)
            .newInstance(target, "msime_account_session_v2");
        Method save = storage.getMethod("save", String.class);
        Method clear = storage.getMethod("clear");
        // 专用 AVD 上的产品没有登录过；有会话时不去覆盖它。
        check(storage.getMethod("load").invoke(v2) == null, "the product already holds a v2 session; use a fresh AVD");
        Object adapter = productAdapter(target);
        long id = 0;
        try {
            save.invoke(v2, new String(sessionBytes(), StandardCharsets.UTF_8));
            JSONObject registered = value(callRaw("sessionStoreRegisterRaw", new Class<?>[] {storeType, byte[].class},
                adapter, null));
            id = registered.getJSONObject("value").getLong("session_store");
            JSONObject status = value(call("accountRaw", new JSONObject().put("session_store", id)
                .put("action", new JSONObject().put("operation", "status"))));
            check(USER.equals(status.getJSONObject("value").getJSONObject("user").getString("id")),
                "Rust did not read the Java v2 session through the product adapter: " + status);
            JSONObject token = value(call("accountAccessTokenRaw", new JSONObject().put("session_store", id)))
                .getJSONObject("value");
            check(hex('a').equals(token.getString("access_token")) && !token.getBoolean("anonymous"),
                "access token through the product adapter: " + token);

            Method load = storeType.getMethod("load", int.class);
            Method put = storeType.getMethod("save", int.class, byte[].class);
            Method remove = storeType.getMethod("remove", int.class);
            // 产品启动时可能已经注册了匿名账号：那时只核对「不覆盖」，不删它的身份。
            byte[] existing = (byte[]) load.invoke(adapter, 4);
            byte[] first = existing != null ? existing
                : "{\"subject\":\"msime-aaaaaaaaaaaaaaaa\",\"secret\":\"first\"}".getBytes(StandardCharsets.UTF_8);
            byte[] second = "{\"subject\":\"msime-bbbbbbbbbbbbbbbb\",\"secret\":\"second\"}".getBytes(StandardCharsets.UTF_8);
            if (existing == null) check((int) put.invoke(adapter, 4, first) == 0, "the first anonymous identity is written");
            check((int) put.invoke(adapter, 4, second) == 1, "a second anonymous identity is not written over the first");
            check(java.util.Arrays.equals(first, (byte[]) load.invoke(adapter, 4)), "the first identity stays");
            if (existing == null) {
                check((int) remove.invoke(adapter, 4) == 0, "the identity is removed");
                check(load.invoke(adapter, 4) == null, "a removed identity reads as absent");
            }
        } finally {
            if (id > 0) callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, id);
            clear.invoke(v2);
        }
    }

    /** 产品的存储适配器（AccountStore 的 Items），和 AccountStore 注册给 Rust 的是同一个类。 */
    private static Object productAdapter(android.content.Context target) throws Exception {
        Class<?> items = target.getClassLoader().loadClass("app.msime.android.AccountStore$Items");
        java.lang.reflect.Constructor<?> constructor = items.getDeclaredConstructor(android.content.Context.class);
        constructor.setAccessible(true);
        return constructor.newInstance(target);
    }

    /**
     * 适配器的第 2 项是云剪贴板密钥串的那个文件（CloudClipboardKeystore，键盘进程也直接读写它）：经适配器存进去的，CloudClipboardKeystore 读得出来，适配器读回的也是它；映射到别的存储时，退出登录后下一个账号会接着用上一个账号的设备私钥和剪贴板密钥。
     *
     * <p>remove(2) 不等密钥串的锁：另一条线程在锁里停住时（端到端加密驱动持着这把锁、经 Rust 核对会话），remove(2) 照样立刻返回 0，文件先留着；锁放开以后文件被删掉。同步删除会在这里一直等到锁放开，产品里那就是 Rust 的会话锁和密钥串的锁互等。
     */
    private void accountStoreKeystore() throws Exception {
        android.content.Context target = getTargetContext();
        ClassLoader loader = target.getClassLoader();
        Class<?> keystoreType = loader.loadClass("app.msime.android.CloudClipboardKeystore");
        Class<?> guardType = loader.loadClass("app.msime.android.CloudClipboardE2e$Guard");
        Object keystore = keystoreType.getMethod("forContext", android.content.Context.class).invoke(null, target);
        Method keystoreLoad = keystoreType.getMethod("load");
        Method keystoreSave = keystoreType.getMethod("save", String.class, guardType);
        Method keystoreDelete = keystoreType.getMethod("delete");
        File file = new File(target.getFilesDir(), "cloud-clipboard-keystore");
        // 专用 AVD 上的产品没有登录过，也就没有密钥串；有的时候不去动它。
        check(!file.exists(), "the product already holds a cloud clipboard keystore; use a fresh AVD");
        Object adapter = productAdapter(target);
        Method load = storeType.getMethod("load", int.class);
        Method put = storeType.getMethod("save", int.class, byte[].class);
        Method remove = storeType.getMethod("remove", int.class);
        String value = "v1.AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA.synthetic-key-id.synthetic-key";
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        CountDownLatch held = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        Throwable[] holderError = new Throwable[1];
        try {
            check((int) put.invoke(adapter, 2, bytes) == 0, "the keystore is saved through the adapter");
            check(value.equals(keystoreLoad.invoke(keystore)), "item 2 is not the cloud clipboard keystore file");
            check(java.util.Arrays.equals(bytes, (byte[]) load.invoke(adapter, 2)), "the adapter reads the keystore back");

            // 在密钥串的锁里停住：save 写入之前在锁里调 guard，同一个值不会撞上身份冲突。
            Object guard = Proxy.newProxyInstance(loader, new Class<?>[] {guardType}, (proxy, method, arguments) -> {
                switch (method.getName()) {
                    case "check":
                        held.countDown();
                        if (!release.await(LOCK_HOLD_SECONDS, TimeUnit.SECONDS))
                            throw new IllegalStateException("the keystore lock was held too long");
                        return null;
                    case "hashCode":
                        return System.identityHashCode(proxy);
                    case "equals":
                        return proxy == arguments[0];
                    default:
                        return "KeystoreLockHolder";
                }
            });
            Thread holder = new Thread(() -> {
                try {
                    keystoreSave.invoke(keystore, value, guard);
                } catch (Throwable error) {
                    holderError[0] = error;
                }
            }, "msime-test-keystore-holder");
            holder.setDaemon(true);
            holder.start();
            check(held.await(10, TimeUnit.SECONDS), "the holder never took the keystore lock");

            long started = android.os.SystemClock.elapsedRealtime();
            int removed = (int) remove.invoke(adapter, 2);
            long elapsed = android.os.SystemClock.elapsedRealtime() - started;
            boolean stillHeld = release.getCount() == 1 && holder.isAlive();
            boolean keptWhileHeld = file.exists();
            release.countDown();
            holder.join(10_000);
            check(removed == 0, "remove(2) returned " + removed);
            check(stillHeld && elapsed < 2_000, "remove(2) waited for the keystore lock (" + elapsed + " ms)");
            check(keptWhileHeld, "the keystore was deleted while another thread held its lock");
            check(!holder.isAlive() && holderError[0] == null, "the lock holder failed: " + holderError[0]);

            long deadline = android.os.SystemClock.elapsedRealtime() + 5_000;
            while (file.exists() && android.os.SystemClock.elapsedRealtime() < deadline) Thread.sleep(20);
            check(!file.exists(), "remove(2) did not delete the keystore file once the lock was released");
            check(keystoreLoad.invoke(keystore) == null && load.invoke(adapter, 2) == null,
                "a removed keystore reads as absent");
        } finally {
            release.countDown();
            keystoreDelete.invoke(keystore);
        }
    }

    /** AccountStore 的两道守卫：不是主进程（`:ime`）时、以及同一进程第二次注册时，都在碰 Rust 之前抛出。 */
    private void accountStoreGuards() throws Exception {
        android.content.Context target = getTargetContext();
        Class<?> store = target.getClassLoader().loadClass("app.msime.android.AccountStore");
        Method register = store.getDeclaredMethod("register", android.content.Context.class, String.class);
        register.setAccessible(true);
        Method id = store.getDeclaredMethod("id", android.content.Context.class);
        id.setAccessible(true);
        expectGuard(register, target, target.getPackageName() + ":ime", "account store owner");
        long first = (long) id.invoke(null, target);
        check(first > 0 && first == (long) id.invoke(null, target), "the main process registers its store once");
        expectGuard(register, target, target.getPackageName(), "account store registered");
        expectGuard(register, target, target.getPackageName() + ":ime", "account store owner");
    }

    /**
     * `:ime` 交给主进程的语音贡献只读 cache 下约定目录里的文件：别处的路径和缺参数在碰 Rust 之前就以 account_invalid 拒绝，别处的文件也不会被删掉。不联网。
     */
    private void voiceContributionPaths() throws Exception {
        android.content.Context target = getTargetContext();
        android.net.Uri uri = android.net.Uri.parse("content://" + target.getPackageName() + ".account-session");
        File outside = new File(target.getCacheDir(), "synthetic-voice-outside.wav");
        java.nio.file.Files.write(outside.toPath(), "not a contribution".getBytes(StandardCharsets.UTF_8));
        try {
            Bundle extras = new Bundle();
            extras.putString("audio_path", outside.getAbsolutePath());
            extras.putString("language", "zh-CN");
            extras.putString("provider", "local");
            extras.putString("transcript", "合成");
            extras.putString("app_version", "1.2.3");
            Bundle reply = target.getContentResolver().call(uri, "voice_contribution", null, extras);
            check(reply != null && "account_invalid".equals(reply.getString("error")),
                "a path outside the contribution directory is refused: " + reply);
            check(outside.isFile(), "a refused path is not deleted");
            Bundle missing = target.getContentResolver().call(uri, "voice_contribution", null, null);
            check(missing != null && "account_invalid".equals(missing.getString("error")),
                "a contribution without arguments is refused: " + missing);
        } finally {
            java.nio.file.Files.deleteIfExists(outside.toPath());
        }
    }

    private static void expectGuard(Method register, android.content.Context target, String process, String message)
            throws Exception {
        try {
            register.invoke(null, target, process);
        } catch (java.lang.reflect.InvocationTargetException thrown) {
            Throwable cause = thrown.getCause();
            check(cause instanceof IllegalStateException && message.equals(cause.getMessage()),
                "register(" + process + ") threw " + cause);
            return;
        }
        throw new AssertionError("register(" + process + ") was not refused");
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
