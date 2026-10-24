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
 * <p>迁移计划 5b 起产品经 AccountStore 注册：它的存储适配器把 Java 原来写的 v2 会话（AndroidKeyStore 加密的 SharedPreferences）原样交给 Rust，匿名身份「不存在才写」，云剪贴板密钥串是 CloudClipboardKeystore 的那个文件，删除是同步的但不等它的锁；AccountStore 只在主进程里注册一次，`:ime` 进程名和第二次注册都在碰 Rust 之前就拒绝。
 *
 * <p>没 attach 的线程上的回调这里造不出来（Rust 自己的线程只在联网刷新时回调存储），由 check-host.sh 在桌面 JVM 上驱动 native/JniCallbacks.cpp 验证。
 */
public final class SessionStoreDeviceSmoke extends Instrumentation {
    private static final String USER = "synthetic-session-store-user";
    /** 冒烟里占着密钥串锁的最长时间；等这把锁的 remove(2) 至少要等这么久才返回。 */
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
        AccountNameChecks.run(this);

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

        settingsPageRequests(delta, progress);
        legacySnapshotQueueActivates();
        legacyCloudSyncImport();
        cloudSyncHostWrites();
    }

    /** Java 旧版队列写 `state.bin` 的格式（DataOutputStream）：魔数、版本 1、本机版本、一个请求。 */
    private static byte[] legacyStateBin(String localVersion, java.util.UUID id, String owner, String expected,
            String sha256, long revision) throws Exception {
        java.io.ByteArrayOutputStream buffer = new java.io.ByteArrayOutputStream();
        try (java.io.DataOutputStream output = new java.io.DataOutputStream(buffer)) {
            output.writeInt(0x4d535131);
            output.writeInt(1);
            output.writeBoolean(true);
            output.writeUTF(localVersion);
            output.writeBoolean(true);
            output.writeLong(id.getMostSignificantBits());
            output.writeLong(id.getLeastSignificantBits());
            output.writeUTF(owner);
            output.writeUTF(expected);
            output.writeUTF(sha256);
            output.writeLong(revision);
            output.writeByte(0);
        }
        return buffer.toByteArray();
    }

    private static String sha256(File file) throws Exception {
        java.security.MessageDigest digest = java.security.MessageDigest.getInstance("SHA-256");
        digest.update(java.nio.file.Files.readAllBytes(file.toPath()));
        StringBuilder text = new StringBuilder(64);
        for (byte value : digest.digest()) text.append(String.format(java.util.Locale.ROOT, "%02x", value));
        return text.toString();
    }

    private static void deleteTree(File root) {
        File[] children = root.listFiles();
        if (children != null) for (File child : children) deleteTree(child);
        if (root.exists() && !root.delete()) throw new AssertionError("could not delete " + root);
    }

    /** 产品的宿主选项（首次准备写下的 runtime-options.json）。 */
    private String productOptions(android.content.Context target) throws Exception {
        File file = new File(target.getFilesDir(), "runtime-options.json");
        check(file.isFile(), "the product has not prepared its host yet");
        return new String(java.nio.file.Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8);
    }

    /**
     * #8201 迁移计划 5d、P13：旧版 Java 队列留下的一份真实 `state.bin`（DataOutputStream 写的，排着一份本地备份恢复的整份快照）经 Rust 的 import_android_v1 转成 `state.json` 以后，键盘空闲时的 DictionarySnapshotWorker（现在调 Rust 的 process）照常认领、准备、激活它。快照是产品词库此刻导出的那一份，所以激活不改变词库的内容；队列放在产品 files 下的一个临时目录里，与真实队列同一个文件系统（激活要原子地换目录）。
     */
    private void legacySnapshotQueueActivates() throws Exception {
        android.content.Context target = getTargetContext();
        ClassLoader loader = target.getClassLoader();
        String options = productOptions(target);
        File queue = new File(target.getFilesDir(), "synthetic-p13-queue-" + System.nanoTime());
        try {
            check(queue.mkdirs(), "queue directory");
            JSONObject version = value(call("snapshotVersionRaw", new JSONObject(options))).getJSONObject("value");
            Object generation = version.opt("generation");
            String local = "local-v1:" + (generation instanceof String text ? text : "legacy") + ":"
                + version.getString("version");
            java.util.UUID id = java.util.UUID.randomUUID();
            File snapshot = new File(queue, id + ".ndjson");
            JSONObject exported = value(call("dictionaryRaw", new JSONObject().put("options", new JSONObject(options))
                .put("action", new JSONObject().put("operation", "export_snapshot")
                    .put("destination", snapshot.getAbsolutePath()))));
            check(snapshot.isFile(), "export_snapshot wrote nothing: " + exported);
            java.nio.file.Files.write(new File(queue, "state.bin").toPath(),
                legacyStateBin(local, id, "local-backup", local, sha256(snapshot), 0));

            Class<?> queueType = loader.loadClass("app.msime.android.DictionarySnapshotQueue");
            Method importV1 = queueType.getMethod("importAndroidV1", java.nio.file.Path.class);
            Method read = queueType.getMethod("read", java.nio.file.Path.class);
            JSONObject imported = (JSONObject) importV1.invoke(null, queue.toPath());
            check(imported.getBoolean("imported"), "the old queue was not converted: " + imported);
            check(!new File(queue, "state.bin").exists() && new File(queue, "state.json").isFile(),
                "state.bin was not replaced by state.json");
            JSONObject request = imported.getJSONObject("state").getJSONObject("request");
            check(id.toString().equals(request.getString("id")) && "queued".equals(request.getString("status"))
                && "local-backup".equals(request.getString("accountId")), "converted request: " + request);
            check(snapshot.isFile(), "the queued snapshot file must stay where it was, not be copied");

            Class<?> worker = loader.loadClass("app.msime.android.DictionarySnapshotWorker");
            Method process = worker.getMethod("process", java.nio.file.Path.class, java.nio.file.Path.class,
                String.class, java.util.function.Supplier.class);
            int[] asked = {0};
            java.util.function.Supplier<String> account = () -> { asked[0]++; return null; };
            // 激活要独占词库维护权：smoke.sh 先停用了本输入法，但系统解绑它、`:ime` 关掉会话是异步的，这之前 Rust 答 snapshot_unavailable，请求留在 preparing、下一次照样能认领（产品里就是等下一次空闲）。所以只对这个码在 30 秒内重试，别的失败和超时照旧抛出。
            long deadline = android.os.SystemClock.uptimeMillis() + 30_000;
            while (true) {
                try {
                    process.invoke(null, queue.toPath(), new File(queue, "staging").toPath(), options, account);
                    break;
                } catch (java.lang.reflect.InvocationTargetException failed) {
                    Throwable cause = failed.getCause();
                    boolean busy = cause != null && "snapshot_unavailable".equals(cause.getMessage())
                        && cause.getClass().getName().equals("app.msime.android.DictionarySnapshotQueue$Failure");
                    if (!busy || android.os.SystemClock.uptimeMillis() >= deadline) throw failed;
                    android.os.SystemClock.sleep(500);
                }
            }
            JSONObject after = (JSONObject) read.invoke(null, queue.toPath());
            JSONObject done = after.optJSONObject("request");
            check(done != null && "applied".equals(done.getString("status")), "the converted snapshot was not activated: " + after);
            check(asked[0] == 0, "a local-backup request must not ask the provider for the account");
            check(!snapshot.exists(), "the activated snapshot file was not removed");
        } finally {
            deleteTree(queue);
        }
    }

    /**
     * 5d 的另一半：旧版 Java 云同步的两份 SharedPreferences 经 RustCloudSync.importLegacy 交给 Rust 的 import_legacy（先把旧快照队列转换好，再带上队列目录）。队列里还有这个账号没应用的快照时，词库游标保留下来；读不懂队列时词库游标不导入。导入以后 SyncSignals 的 peek 看到开关开着、mark_dirty 记得下改动；没导入的全新状态里两者都是关着的。DictionarySnapshotWorker 遇到属于账号的请求时才问 provider 当前账号，问不到（null）就原样留着；退出登录的 cancel 取消它。都在临时目录里做，不碰产品的同步状态。
     */
    private void legacyCloudSyncImport() throws Exception {
        android.content.Context target = getTargetContext();
        ClassLoader loader = target.getClassLoader();
        Class<?> sync = loader.loadClass("app.msime.android.RustCloudSync");
        Class<?> signals = loader.loadClass("app.msime.android.SyncSignals");
        Class<?> queueType = loader.loadClass("app.msime.android.DictionarySnapshotQueue");
        Method preferences = sync.getDeclaredMethod("preferences", Map.class);
        preferences.setAccessible(true);
        Method importLegacy = sync.getDeclaredMethod("importLegacy", String.class, JSONObject.class, JSONObject.class,
            java.nio.file.Path.class);
        importLegacy.setAccessible(true);
        Method markDirty = signals.getDeclaredMethod("markDirty", String.class, String.class);
        markDirty.setAccessible(true);
        Method enabled = signals.getDeclaredMethod("enabled", String.class);
        enabled.setAccessible(true);
        Method peek = signals.getDeclaredMethod("call", String.class, JSONObject.class, JSONObject.class);
        peek.setAccessible(true);
        Method importV1 = queueType.getMethod("importAndroidV1", java.nio.file.Path.class);
        Method read = queueType.getMethod("read", java.nio.file.Path.class);
        Method cancel = queueType.getMethod("cancel", java.nio.file.Path.class, String.class);

        String account = "synthetic-sync-account";
        Map<String, Object> legacy = new HashMap<>();
        legacy.put("enabled", true);
        legacy.put("account_id", account);
        legacy.put("login_kind", "email");
        legacy.put("binding_generation", 4L);
        legacy.put("cursor_settings", "3");
        legacy.put("gen_settings", 2L);
        legacy.put("clean_settings", 1L);
        legacy.put("cursor_dictionary", "7");
        legacy.put("phrases_unheld", new java.util.HashSet<>(java.util.List.of("收不下的常用语")));
        legacy.put("last_synced_at", 1_700_000_000_000L);
        Map<String, Object> status = new HashMap<>();
        status.put("binding_generation", 4L);
        status.put("last_attempt", 1_700_000_000_000L);
        JSONObject syncV1 = (JSONObject) preferences.invoke(null, legacy);
        check(syncV1.getJSONArray("phrases_unheld").length() == 1 && syncV1.getLong("gen_settings") == 2L,
            "SharedPreferences values were not written as JSON: " + syncV1);
        JSONObject statusV1 = (JSONObject) preferences.invoke(null, status);

        File root = new File(target.getCacheDir(), "synthetic-cloud-sync-" + System.nanoTime());
        try {
            File queue = new File(root, "dictionary-snapshots");
            check(queue.mkdirs(), "queue directory");
            String local = "local-v1:legacy:" + hex('a');
            java.util.UUID id = java.util.UUID.randomUUID();
            File snapshot = new File(queue, id + ".ndjson");
            java.nio.file.Files.write(snapshot.toPath(), "synthetic snapshot\n".getBytes(StandardCharsets.UTF_8));
            java.nio.file.Files.write(new File(queue, "state.bin").toPath(),
                legacyStateBin(local, id, account, local, sha256(snapshot), 7));

            File fresh = new File(root, "fresh");
            check(!(Boolean) enabled.invoke(null, fresh.getAbsolutePath()), "a fresh state reads as off");
            check(!(Boolean) markDirty.invoke(null, fresh.getAbsolutePath(), "phrases"), "nothing is marked while off");

            // 与产品启动时的顺序相同：先转换旧队列，再导入旧同步进度。
            JSONObject converted = (JSONObject) importV1.invoke(null, queue.toPath());
            check(converted.getBoolean("imported"), "convert the old queue first: " + converted);
            File state = new File(root, "state");
            check((Boolean) importLegacy.invoke(null, state.getAbsolutePath(), syncV1, statusV1, queue.toPath()),
                "import_legacy did not answer");
            JSONObject imported = ((JSONObject) peek.invoke(null, state.getAbsolutePath(),
                new JSONObject().put("operation", "peek"), null));
            check(imported.getBoolean("enabled") && account.equals(imported.getString("account_id")),
                "the old switch and account were not imported: " + imported);
            JSONObject sections = imported.getJSONObject("sections");
            check(sections.getJSONObject("settings").getInt("cursor") == 3
                && sections.getJSONObject("settings").getBoolean("dirty"), "settings progress: " + sections);
            check(sections.getJSONObject("dictionary").getInt("cursor") == 7,
                "a dictionary cursor with a queued snapshot for the account is kept: " + sections);
            check(imported.getInt("unheld_phrases") == 1, "unheld phrases: " + imported);
            check((Boolean) enabled.invoke(null, state.getAbsolutePath()), "peek reads the imported switch");
            check((Boolean) markDirty.invoke(null, state.getAbsolutePath(), "phrases"), "a local edit is marked");
            check(((JSONObject) peek.invoke(null, state.getAbsolutePath(), new JSONObject().put("operation", "peek"), null))
                .getJSONObject("sections").getJSONObject("phrases").getBoolean("dirty"), "the phrases mark is in Rust's state");

            // 读不懂的旧队列：词库游标不导入，第一轮按首次同步处理。
            File broken = new File(root, "broken-queue");
            check(broken.mkdirs(), "broken queue directory");
            java.nio.file.Files.write(new File(broken, "state.bin").toPath(), new byte[] {1, 2, 3});
            File other = new File(root, "other-state");
            check((Boolean) importLegacy.invoke(null, other.getAbsolutePath(), syncV1, statusV1, broken.toPath()),
                "import_legacy did not answer for an unreadable queue");
            JSONObject unknown = (JSONObject) peek.invoke(null, other.getAbsolutePath(),
                new JSONObject().put("operation", "peek"), null);
            check(unknown.getJSONObject("sections").getJSONObject("dictionary").isNull("cursor")
                && unknown.getJSONObject("sections").getJSONObject("settings").getInt("cursor") == 3,
                "an unreadable queue must drop only the dictionary cursor: " + unknown);

            // 属于账号的请求：不知道当前账号时原样留着（只问一次 provider），退出登录时取消。
            Class<?> worker = loader.loadClass("app.msime.android.DictionarySnapshotWorker");
            Method process = worker.getMethod("process", java.nio.file.Path.class, java.nio.file.Path.class,
                String.class, java.util.function.Supplier.class);
            int[] asked = {0};
            java.util.function.Supplier<String> unknownAccount = () -> { asked[0]++; return null; };
            process.invoke(null, queue.toPath(), new File(queue, "staging").toPath(), productOptions(target), unknownAccount);
            check(asked[0] == 1, "an account-owned request asks for the current account once: " + asked[0]);
            check("queued".equals(((JSONObject) read.invoke(null, queue.toPath())).getJSONObject("request").getString("status")),
                "a request for an unknown account must stay queued");
            JSONObject cancelled = (JSONObject) cancel.invoke(null, queue.toPath(), account);
            check("cancelled".equals(cancelled.getJSONObject("request").getString("status")) && !snapshot.exists(),
                "sign-out cancels the account's queued snapshot: " + cancelled);
        } finally {
            deleteTree(root);
        }
    }

    /**
     * 5d：一轮回答 `pending_commit` 时，RustCloudSync.commit 在账号绑定锁里先核对 `status.account_id` 是不是当前登录的账号，再把 `host_writes` 写进四个本机存储，最后以 `run_id` 提交。这里的 `host_writes` 按 Rust 的 HostWrites 拼（`feedback` 的 soundEnabled、hapticsEnabled、hapticStrength，`android_local`，`custom_keyboard_skins`，`adopt_texts`），产品读错一个键名时对应的存储不变，这里就失败；不然那一项会被静默跳过、照样提交，游标前进以后云端的值再也写不回这台设备。
     *
     * <p>四个存储都放在临时目录里：ContextWrapper 把 files 目录换成临时的，按键反馈和 Android 本地设置跟着它走，`runtime-options.json` 里的偏好目录（常用语的示例标记）和传给 commit 的偏好目录（皮肤库）也指向临时目录，同步状态目录同样在它下面。专用 AVD 没有登录，当前账号是空串：账号对不上时什么也不写、也不提交（提交会抛出 stale_run）；对得上时四项都写对，随后的提交带着 `run_id` 和偏好目录到了 Rust，Rust 没有这一轮的待提交进度，回答 stale_run（少了 `run_id` 或偏好目录会是 cloud_sync_invalid）。
     */
    private void cloudSyncHostWrites() throws Exception {
        android.content.Context target = getTargetContext();
        ClassLoader loader = target.getClassLoader();
        // 产品的会话存储要用产品自己的 Context 注册；下面的 ContextWrapper 只换目录。
        Method storeId = loader.loadClass("app.msime.android.AccountStore").getDeclaredMethod("id", android.content.Context.class);
        storeId.setAccessible(true);
        storeId.invoke(null, target);
        Class<?> account = loader.loadClass("app.msime.android.RustAccount");
        check("".equals(account.getMethod("userId", android.content.Context.class).invoke(null, target)),
            "the product is signed in; use a fresh AVD");
        Class<?> sync = loader.loadClass("app.msime.android.RustCloudSync");
        Method commit = sync.getDeclaredMethod("commit", android.content.Context.class, String.class, JSONObject.class);
        commit.setAccessible(true);
        Class<?> feedbackStore = loader.loadClass("app.msime.android.KeyboardFeedbackStore");
        Method loadFeedback = feedbackStore.getMethod("load", android.content.Context.class);
        Class<?> localSettings = loader.loadClass("app.msime.android.AndroidLocalSettings");
        Method loadLocal = localSettings.getMethod("load", android.content.Context.class);
        Class<?> snapshot = loader.loadClass("app.msime.android.AndroidLocalSettings$Snapshot");
        Method bool = snapshot.getMethod("bool", String.class);
        Method integer = snapshot.getMethod("integer", String.class);
        String fallback = (String) localSettings.getField("VOICE_OFFLINE_FALLBACK").get(null);
        String strokeWidth = (String) localSettings.getField("HANDWRITING_STROKE_WIDTH").get(null);
        Class<?> skins = loader.loadClass("app.msime.android.CustomSkinLibrary");
        Method readSkins = skins.getMethod("read", java.nio.file.Path.class);
        Class<?> phrases = loader.loadClass("app.msime.android.CommonPhrasesStore");
        Method untouched = phrases.getMethod("untouchedStarters", android.content.Context.class);
        Method editStarters = phrases.getDeclaredMethod("editStarters", File.class, boolean.class,
            java.util.function.UnaryOperator.class);
        editStarters.setAccessible(true);
        java.lang.reflect.Field markerName = phrases.getDeclaredField("STARTER_MARKER");
        markerName.setAccessible(true);

        File root = new File(target.getCacheDir(), "synthetic-host-writes-" + System.nanoTime());
        try {
            File files = new File(root, "files");
            File preferences = new File(root, "preferences");
            check(files.mkdirs() && preferences.mkdirs(), "temporary directories");
            java.nio.file.Files.write(new File(files, "runtime-options.json").toPath(),
                new JSONObject().put("preferences_directory", preferences.getAbsolutePath()).toString()
                    .getBytes(StandardCharsets.UTF_8));
            android.content.Context isolated = new android.content.ContextWrapper(target) {
                @Override public File getFilesDir() { return files; }
                @Override public android.content.Context getApplicationContext() { return this; }
            };
            String kept = "合成示例甲";
            String adopted = "合成示例乙";
            editStarters.invoke(null, new File(preferences, (String) markerName.get(null)), true,
                (java.util.function.UnaryOperator<java.util.Set<String>>) current -> new java.util.LinkedHashSet<>(
                    java.util.List.of(kept, adopted)));
            check(java.util.Set.of(kept, adopted).equals(untouched.invoke(null, isolated)), "planted starters");

            JSONObject writes = new JSONObject()
                .put("feedback", new JSONObject().put("soundEnabled", false).put("hapticsEnabled", true)
                    .put("hapticStrength", "strong"))
                .put("android_local", new JSONObject().put(fallback, false).put(strokeWidth, 5))
                .put("custom_keyboard_skins", new JSONArray().put(new JSONObject().put("id", "synthetic-sync-skin")
                    .put("name", "同步皮肤").put("updated_at", 100)
                    .put("design", new JSONObject().put("accent", 0x123456))).toString())
                .put("adopt_texts", new JSONArray().put(adopted));
            String directory = preferences.getAbsolutePath();

            // 联网那段时间里换了号：这一轮下载的是别的账号的值，不写、不提交。
            commit.invoke(null, isolated, directory, pendingCommit("synthetic-other-account", writes));
            Object feedback = loadFeedback.invoke(null, isolated);
            check(Boolean.TRUE.equals(feedback.getClass().getMethod("soundEnabled").invoke(feedback))
                && Boolean.FALSE.equals(feedback.getClass().getMethod("hapticsEnabled").invoke(feedback)),
                "another account's feedback was written");
            Object local = loadLocal.invoke(null, isolated);
            check((Boolean) bool.invoke(local, fallback) && (Integer) integer.invoke(local, strokeWidth) == 3,
                "another account's local settings were written");
            check(((java.util.List<?>) readSkins.invoke(null, preferences.toPath())).isEmpty(),
                "another account's skins were written");
            check(java.util.Set.of(kept, adopted).equals(untouched.invoke(null, isolated)),
                "another account's phrases adopted a starter");

            // 账号对得上：四项都写进本机，再以 run_id 提交。
            try {
                commit.invoke(null, isolated, directory, pendingCommit("", writes));
                throw new AssertionError("a commit without a pending round was accepted");
            } catch (java.lang.reflect.InvocationTargetException thrown) {
                Throwable cause = thrown.getCause();
                Object code = cause.getClass().getName().endsWith("AccountStore$Failure")
                    ? cause.getClass().getField("code").get(cause) : null;
                check("stale_run".equals(code), "the commit did not reach Rust with the run id: " + cause);
            }
            feedback = loadFeedback.invoke(null, isolated);
            Object strength = feedback.getClass().getMethod("hapticStrength").invoke(feedback);
            check(Boolean.FALSE.equals(feedback.getClass().getMethod("soundEnabled").invoke(feedback))
                && Boolean.TRUE.equals(feedback.getClass().getMethod("hapticsEnabled").invoke(feedback))
                && "strong".equals(strength.getClass().getMethod("id").invoke(strength)),
                "host_writes.feedback was not written: " + feedback);
            local = loadLocal.invoke(null, isolated);
            check(!(Boolean) bool.invoke(local, fallback) && (Integer) integer.invoke(local, strokeWidth) == 5,
                "host_writes.android_local was not written");
            java.util.List<?> library = (java.util.List<?>) readSkins.invoke(null, preferences.toPath());
            check(library.size() == 1 && "synthetic-sync-skin".equals(library.get(0).getClass().getMethod("id")
                .invoke(library.get(0))), "host_writes.custom_keyboard_skins was not written: " + library);
            check(java.util.Set.of(kept).equals(untouched.invoke(null, isolated)),
                "host_writes.adopt_texts did not adopt the starter");
        } finally {
            deleteTree(root);
        }
    }

    /** RustCloudSync.run 拿到的一轮 `pending_commit` 回答，`account` 是这一轮绑定的账号。 */
    private static JSONObject pendingCommit(String account, JSONObject writes) throws Exception {
        return new JSONObject().put("outcome", "pending_commit").put("run_id", "synthetic-run-" + System.nanoTime())
            .put("status", new JSONObject().put("account_id", account)).put("host_writes", new JSONObject(writes.toString()));
    }

    /**
     * 5c 起设置页面经 Rust 发的那几种请求（#8201 迁移计划）：用一份没有任何会话的内存存储，Rust 在联网之前就按请求的形状作答——形状不对是「请求不合法」，形状对了是没有会话（account_unauthorized、community_unauthorized）。所以这里不联网，核对的是页面拼的请求 Rust 认得，以及页面在本机先做的那几项检查与 Rust 的上限一致。
     */
    private void settingsPageRequests(Class<?> delta, Class<?> progress) throws Exception {
        long empty = register(new MemoryStore(false), null);
        ClassLoader loader = client.getClassLoader();

        // 流式对话：Rust 要求带 model，所以对话页先读 chat_models 的 defaultModel。
        JSONArray messages = new JSONArray().put(new JSONObject().put("role", "system").put("content", "合成提示"))
            .put(new JSONObject().put("role", "user").put("content", "shuishan"));
        JSONObject chat = new JSONObject().put("session_store", empty).put("request_id", "synthetic-chat")
            .put("messages", messages);
        expectError(callRaw("accountChatStreamRaw", new Class<?>[] {byte[].class, delta}, bytes(chat), null),
            "account_invalid");
        expectError(callRaw("accountChatStreamRaw", new Class<?>[] {byte[].class, delta},
            bytes(new JSONObject(chat.toString()).put("model", "synthetic-model")), null), "account_unauthorized");

        // AI 皮肤：页面查过的描述 Rust 也收；没有会话时是 account_unauthorized，RustAccount 据此先注册匿名账号。
        Class<?> skins = loader.loadClass("app.msime.android.SkinJobsApi");
        String prompt = "秋天的银杏";
        check((boolean) skins.getMethod("validPrompt", String.class).invoke(null, prompt), "page accepts the prompt");
        expectError(callRaw("aiSkinGenerateRaw", new Class<?>[] {byte[].class, progress}, bytes(new JSONObject()
            .put("session_store", empty).put("request_id", "synthetic-skin").put("prompt", prompt)), null),
            "account_unauthorized");

        // 删除云端数据：页面的分区表（含 voice）Rust 全认；不认识的分区是 account_invalid。
        @SuppressWarnings("unchecked")
        java.util.List<String> sections = (java.util.List<String>) loader.loadClass("app.msime.android.DeviceDataApi")
            .getField("DELETABLE_SECTIONS").get(null);
        expectError(call("accountRaw", new JSONObject().put("session_store", empty).put("action", new JSONObject()
            .put("operation", "data_delete").put("sections", new JSONArray(sections)))), "account_unauthorized");
        expectError(call("accountRaw", new JSONObject().put("session_store", empty).put("action", new JSONObject()
            .put("operation", "data_delete").put("sections", new JSONArray().put("synthetic")))), "account_invalid");

        // 诊断上传：与开发者选项拼的 bundle 同形；一类都没勾时 Rust 不读文件就拒绝。
        java.io.File root = new java.io.File(getTargetContext().getCacheDir(), "synthetic-diagnostics-upload");
        if (!root.isDirectory() && !root.mkdirs()) throw new AssertionError("diagnostics directory");
        JSONObject sources = new JSONObject().put("crash_logs", JSONObject.NULL)
            .put("performance_logs", JSONObject.NULL).put("input_events", JSONObject.NULL);
        JSONObject upload = new JSONObject().put("operation", "diagnostics_upload").put("platform", "android")
            .put("app_version", "1.2.3").put("ttl", "one_day").put("bundle", new JSONObject()
                .put("state_root", root.getAbsolutePath()).put("sources", sources).put("include", new JSONObject()
                    .put("crash_logs", true).put("performance_logs", false).put("input_events", false)
                    .put("config_snapshot", false)));
        expectError(call("accountRaw", new JSONObject().put("session_store", empty).put("action", upload)),
            "account_unauthorized");
        upload.getJSONObject("bundle").getJSONObject("include").put("crash_logs", false);
        expectError(call("accountRaw", new JSONObject().put("session_store", empty).put("action", upload)),
            "account_invalid");

        // 反馈截图：Rust 只读文件头和尺寸，单边超过 8192 是 account_too_large；FeedbackApi.fitsRust 在同一处划线。
        Method fits = loader.loadClass("app.msime.android.FeedbackApi")
            .getMethod("fitsRust", long.class, int.class, int.class);
        for (int width : new int[] {8192, 8193}) {
            java.io.File shot = new java.io.File(root, "synthetic-" + width + ".jpg");
            android.graphics.Bitmap bitmap = android.graphics.Bitmap.createBitmap(width, 1,
                android.graphics.Bitmap.Config.ARGB_8888);
            try (java.io.OutputStream output = new java.io.FileOutputStream(shot)) {
                check(bitmap.compress(android.graphics.Bitmap.CompressFormat.JPEG, 80, output), "synthetic screenshot");
            } finally {
                bitmap.recycle();
            }
            boolean fitsJava = (boolean) fits.invoke(null, shot.length(), width, 1);
            byte[] answer = call("accountRaw", new JSONObject().put("session_store", empty).put("action", new JSONObject()
                .put("operation", "feedback").put("type", "bug").put("text", "合成反馈").put("platform", "android")
                .put("app_version", "1.2.3").put("edition", "full")
                .put("screenshots", new JSONArray().put(shot.getAbsolutePath()))));
            expectError(answer, fitsJava ? "account_unauthorized" : "account_too_large");
            check(fitsJava == (width <= 8192), "the page and Rust draw the screenshot edge at the same place: " + width);
            if (!shot.delete()) shot.deleteOnExit();
        }
        if (!root.delete()) root.deleteOnExit();

        // 社区：下载皮肤没有任何会话时是 community_unauthorized，RustAccount 据此注册匿名账号再试；举报的原因不在固定的几条里是 community_invalid。
        String skin = "10000000-0000-4000-8000-000000000001";
        expectError(call("communityRaw", new JSONObject().put("session_store", empty).put("action", new JSONObject()
            .put("operation", "keyboard_skin_download").put("id", skin))), "community_unauthorized");
        expectError(call("communityRaw", new JSONObject().put("session_store", empty).put("action", new JSONObject()
            .put("operation", "report").put("kind", "skins").put("id", skin).put("reason", "合成原因"))),
            "community_invalid");

        // 云剪贴板不涉及内容的操作：没有登录时是 account_unauthorized，保留时长只认 0、1、7、30。
        expectError(call("accountCloudClipboardRaw", new JSONObject().put("session_store", empty)
            .put("action", new JSONObject().put("operation", "set_retention").put("days", 7))), "account_unauthorized");
        expectError(call("accountCloudClipboardRaw", new JSONObject().put("session_store", empty)
            .put("action", new JSONObject().put("operation", "set_retention").put("days", 3))), "invalid_cloud_clipboard");

        check(value(callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, empty)).getBoolean("value"),
            "unregister of the empty store");
    }

    /**
     * 产品的存储适配器（AccountStore 的 Items）原样交出 Java 原来写的 v2 会话：共享用例 `shared/contracts/account-session-layouts/cases.json` 的 `android_v2`（构建时打进测试包的 assets）用 AndroidAccountSessionStorage 加密写进 `msime_account_session_v2`，再把适配器另外注册一次，Rust 的 status 读出用例里的账号。用例的过期时间在过去，status 不刷新；要拿访问令牌时由 Android 的 org.json 把它改成一小时以后再写一份、重新注册（flags=0 的存储 Rust 读一次就缓存），先核对 org.json 写回的字节与用例逐字相同，用例确实是这台设备上 Java 写出的形状。匿名身份的 save 是「不存在才写」。用到的项最后都删掉。
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
        JSONObject layout = layoutCase("android_v2");
        String document = layout.getString("document");
        JSONObject expected = layout.getJSONObject("session").getJSONObject("tokens");
        check(document.equals(new JSONObject(document).toString()),
            "the android_v2 case is not what this device's org.json writes: " + new JSONObject(document));
        Object adapter = productAdapter(target);
        long id = 0;
        try {
            save.invoke(v2, document);
            id = registerProductAdapter(adapter);
            JSONObject status = value(call("accountRaw", new JSONObject().put("session_store", id)
                .put("action", new JSONObject().put("operation", "status"))));
            check(expected.getJSONObject("user").getString("id")
                    .equals(status.getJSONObject("value").getJSONObject("user").getString("id")),
                "Rust did not read the Java v2 session through the product adapter: " + status);
            callRaw("sessionStoreUnregisterRaw", new Class<?>[] {long.class}, id);
            id = 0;

            save.invoke(v2, new JSONObject(document)
                .put("expires_at_unix_ms", System.currentTimeMillis() + 3_600_000L).toString());
            id = registerProductAdapter(adapter);
            JSONObject token = value(call("accountAccessTokenRaw", new JSONObject().put("session_store", id)))
                .getJSONObject("value");
            check(expected.getString("access_token").equals(token.getString("access_token"))
                    && !token.getBoolean("anonymous"),
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

    private long registerProductAdapter(Object adapter) throws Exception {
        return value(callRaw("sessionStoreRegisterRaw", new Class<?>[] {storeType, byte[].class}, adapter, null))
            .getJSONObject("value").getLong("session_store");
    }

    /** 共享用例 account-session-layouts 里名为 name 的那一条；build-editor.sh 把用例文件打进测试包的 assets。 */
    private JSONObject layoutCase(String name) throws Exception {
        java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
        // InputStream.readAllBytes 要 API 33，设备套件也跑 API 28。
        try (java.io.InputStream input = getContext().getAssets().open("contracts/account-session-layouts/cases.json")) {
            byte[] buffer = new byte[8192];
            for (int read; (read = input.read(buffer)) > 0; ) bytes.write(buffer, 0, read);
        }
        JSONArray cases = new JSONObject(new String(bytes.toByteArray(), StandardCharsets.UTF_8)).getJSONArray("cases");
        for (int index = 0; index < cases.length(); index++) {
            if (name.equals(cases.getJSONObject(index).getString("name"))) return cases.getJSONObject(index);
        }
        throw new AssertionError("no layout case named " + name);
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
     * <p>remove(2) 是同步的，但不等密钥串的锁：另一条线程在锁里停住时（端到端加密驱动持着这把锁、经 Rust 核对会话），remove(2) 照样很快返回 0，返回时文件已经删掉；那条线程的保存随后以 CancellationException 结束，不会把旧身份写回去。等这把锁的删除会在这里一直等到锁放开，产品里那就是 Rust 的会话锁和密钥串的锁互等；起线程去删的删除返回时文件还在。
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
            boolean goneOnReturn = !file.exists();
            release.countDown();
            holder.join(10_000);
            Throwable holderCause = holderError[0] instanceof java.lang.reflect.InvocationTargetException wrapped
                ? wrapped.getCause() : holderError[0];
            check(removed == 0, "remove(2) returned " + removed);
            check(stillHeld && elapsed < 2_000, "remove(2) waited for the keystore lock (" + elapsed + " ms)");
            check(goneOnReturn, "remove(2) returned before the keystore file was deleted");
            check(!holder.isAlive() && holderCause instanceof java.util.concurrent.CancellationException,
                "a save that started before remove(2) was not refused: " + holderCause);
            check(!file.exists(), "a save that started before remove(2) wrote the keystore back");
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
