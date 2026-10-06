package app.msime.android;

import java.nio.file.Files;
import java.nio.file.Path;
import org.json.JSONObject;

/** Runs snapshot preparation and activation only after the IME session is gone. */
public final class DictionarySnapshotWorker {
    private DictionarySnapshotWorker() {}

    public static void process(Path filesRoot, Path queueDirectory, Path stagingDirectory, String options)
            throws Exception {
        DictionarySnapshotQueue queue = new DictionarySnapshotQueue(filesRoot, queueDirectory);
        String current = version(options);
        queue.publishLocalVersion(current);
        try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
            DictionarySnapshotQueue.Request request = queue.claim(lease);
            if (request == null) return;
            long handle = 0;
            try {
                ensureSafeDirectory(stagingDirectory);
                String prepareRequest = new JSONObject()
                    .put("options", new JSONObject(options))
                    .put("staging_root", stagingDirectory.toAbsolutePath().normalize().toString())
                    .put("expected_version", request.expectedLocalVersion())
                    .put("activation_id", request.id().toString())
                    .put("records", 0)
                    .toString();
                JSONObject prepared = new JSONObject(NativeClient.snapshotPrepare(
                    prepareRequest, queue.filePath(request.id()).toString()));
                if (!Boolean.TRUE.equals(strictBoolean(prepared.opt("ok")))) {
                    queue.fail(request.id(), lease);
                    return;
                }
                long preparedHandle = DictionarySnapshotPolicy.handle(
                    prepared.getJSONObject("value").opt("handle"), 0);
                if (preparedHandle == 0)
                    throw new IllegalStateException("snapshot handle invalid");
                handle = preparedHandle;
                boolean applied = queue.complete(request.id(), lease, current, false, () -> {
                    JSONObject activated = new JSONObject(NativeClient.snapshotActivate(
                        preparedHandle, request.expectedLocalVersion()));
                    if (!Boolean.TRUE.equals(strictBoolean(activated.opt("ok"))))
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
        if (!Boolean.TRUE.equals(strictBoolean(result.opt("ok"))))
            throw new IllegalStateException("snapshot version unavailable");
        JSONObject value = result.getJSONObject("value");
        String digest = value.getString("version");
        String generation = value.optString("generation", "legacy");
        String version = "local-v1:" + generation + ":" + digest;
        if (!DictionarySnapshotQueue.validVersion(version))
            throw new IllegalStateException("snapshot version invalid");
        return version;
    }

    /** Snapshot bridge status flags must remain JSON booleans; reject scalar coercion. */
    static Boolean strictBoolean(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
    }
}
