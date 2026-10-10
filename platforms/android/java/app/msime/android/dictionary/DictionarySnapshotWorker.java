package app.msime.android;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.function.Supplier;
import org.json.JSONException;
import org.json.JSONObject;

/** Runs snapshot preparation and activation only after the IME session is gone. */
public final class DictionarySnapshotWorker {
    private DictionarySnapshotWorker() {}

    /** 先处理排着的整份快照激活，再合并本地备份恢复时排下的输入记录。合并会改本机词库版本，放在激活前面会让排着的激活因版本不符被拒。 */
    public static void process(Path filesRoot, Path queueDirectory, Path stagingDirectory, String options,
            Supplier<String> currentAccountId)
            throws Exception {
        try {
            activate(filesRoot, queueDirectory, stagingDirectory, options, currentAccountId);
        } finally {
            mergePendingLearning(options);
        }
    }

    /**
     * 合并本地备份恢复时排下的输入记录（`merge_pending_learning`，#5659）。这里键盘已经没有会话，拿得到独占维护权；不放在建会话前的个人词库同步里，免得一份大备份拖慢恢复后第一次弹出键盘。没有待合并的文件时原生侧什么也不做；失败时原生侧保留文件、记下次数，下次空闲再试，连续失败几次后放弃，所以这里不看结果。
     */
    static void mergePendingLearning(String options) {
        try {
            NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(options))
                .put("action", new JSONObject().put("operation", "merge_pending_learning"))
                .toString());
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 下次空闲再试；快照处理不能因为它失败。
        }
    }

    private static void activate(Path filesRoot, Path queueDirectory, Path stagingDirectory, String options,
            Supplier<String> currentAccountId) throws Exception {
        DictionarySnapshotQueue queue = new DictionarySnapshotQueue(filesRoot, queueDirectory);
        String current = version(options);
        queue.publishLocalVersion(current);
        try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
            DictionarySnapshotQueue.Request request = queue.claim(lease, currentAccountId);
            if (request == null) return;
            long handle = 0;
            try {
                ensureSafeDirectory(stagingDirectory);
                // 队列里记的是带前缀的本机版本，原生侧只认其中的摘要。
                String expected = DictionarySnapshotQueue.nativeVersion(request.expectedLocalVersion());
                String prepareRequest = new JSONObject()
                    .put("options", new JSONObject(options))
                    .put("staging_root", stagingDirectory.toAbsolutePath().normalize().toString())
                    .put("expected_version", expected)
                    .put("activation_id", request.id().toString())
                    .put("records", 0)
                    .toString();
                JSONObject prepared = new JSONObject(NativeClient.snapshotPrepare(
                    prepareRequest, queue.filePath(request.id()).toString()));
                if (!JsonPolicy.strictTrue(prepared.opt("ok"))) {
                    queue.fail(request.id(), lease);
                    return;
                }
                long preparedHandle = DictionarySnapshotPolicy.handle(
                    prepared.getJSONObject("value").opt("handle"), 0);
                if (preparedHandle == 0)
                    throw new IllegalStateException("snapshot handle invalid");
                handle = preparedHandle;
                boolean applied = queue.complete(request.id(), lease, current, false, currentAccountId, () -> {
                    JSONObject activated = new JSONObject(NativeClient.snapshotActivate(
                        preparedHandle, expected));
                    if (!JsonPolicy.strictTrue(activated.opt("ok")))
                        throw new IllegalStateException("snapshot activation rejected");
                    return version(options);
                });
                if (!applied) {
                    try { NativeClient.snapshotDiscard(preparedHandle); }
                    finally { handle = 0; }
                } else handle = 0;
            } catch (Exception error) {
                try { queue.fail(request.id(), lease); }
                catch (Exception ignored) { /* Preserve the original worker failure. */ }
                throw error;
            } finally {
                if (handle != 0) NativeClient.snapshotDiscard(handle);
            }
        }
    }

    static void ensureSafeDirectory(Path directory) throws java.io.IOException {
        if (directory == null) throw new java.io.IOException("snapshot staging directory unavailable");
        SafePaths.ensureDirectory(directory);
    }

    private static String version(String options) throws Exception {
        JSONObject result = new JSONObject(NativeClient.snapshotVersion(options));
        if (!JsonPolicy.strictTrue(result.opt("ok")))
            throw new IllegalStateException("snapshot version unavailable");
        JSONObject value = result.getJSONObject("value");
        String digest = JsonPolicy.strictString(value.opt("version"));
        if (digest == null) throw new IllegalStateException("snapshot version missing");
        Object rawGeneration = value.opt("generation");
        String generation = rawGeneration == null || rawGeneration == JSONObject.NULL
            ? "legacy" : JsonPolicy.strictString(rawGeneration);
        if (generation == null) throw new IllegalStateException("snapshot generation invalid");
        String version = "local-v1:" + generation + ":" + digest;
        if (!DictionarySnapshotQueue.validVersion(version))
            throw new IllegalStateException("snapshot version invalid");
        return version;
    }

}
