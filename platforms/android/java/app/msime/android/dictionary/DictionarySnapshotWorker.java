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
                if (!prepared.optBoolean("ok", false)) {
                    queue.fail(request.id(), lease);
                    return;
                }
                handle = prepared.getJSONObject("value").getLong("handle");
                final long preparedHandle = handle;
                boolean applied = queue.complete(request.id(), lease, current, false, () -> {
                    JSONObject activated = new JSONObject(NativeClient.snapshotActivate(
                        preparedHandle, request.expectedLocalVersion()));
                    if (!activated.optBoolean("ok", false))
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
        Path absolute = directory.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new java.io.IOException("snapshot staging directory unavailable");
        for (Path component : absolute) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current))
                throw new java.io.IOException("snapshot staging path contains a symbolic link");
        }
        if (Files.exists(absolute, java.nio.file.LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(absolute, java.nio.file.LinkOption.NOFOLLOW_LINKS))
            throw new java.io.IOException("snapshot staging directory unavailable");
        Files.createDirectories(absolute);
        current = absolute.getRoot();
        for (Path component : absolute) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current))
                throw new java.io.IOException("snapshot staging path contains a symbolic link");
        }
        if (!Files.isDirectory(absolute, java.nio.file.LinkOption.NOFOLLOW_LINKS))
            throw new java.io.IOException("snapshot staging directory unavailable");
    }

    private static String version(String options) throws Exception {
        JSONObject result = new JSONObject(NativeClient.snapshotVersion(options));
        if (!result.optBoolean("ok", false)) throw new IllegalStateException("snapshot version unavailable");
        JSONObject value = result.getJSONObject("value");
        String digest = value.getString("version");
        String generation = value.optString("generation", "legacy");
        String version = "local-v1:" + generation + ":" + digest;
        if (!DictionarySnapshotQueue.validVersion(version))
            throw new IllegalStateException("snapshot version invalid");
        return version;
    }
}
