package app.msime.android;

import android.content.Context;
import java.io.File;
import java.io.IOException;
import java.nio.file.Path;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 词库快照激活队列的 Java 一侧（#8201 迁移计划 5d）：状态、排队、取消、认领和激活都在 Rust 的 {@code msime_client_snapshot_queue} 里，在队列目录的 `state.lock` 上用 flock 互斥。这里只拼请求、读回答。
 *
 * <p>原来这里是 Java 自己的一份实现，状态写在 `state.bin`、用 Java 的文件锁（fcntl）加锁，与 Rust 的 flock 互相看不见，所以主进程（云同步、本地备份、Tauri 合包的插件、退出登录）和 `:ime`（空闲时激活）在同一组提交里一起改调 Rust。旧版本留下的 `state.bin` 由 Rust 就地转成 `state.json`（{@link #read}，主进程启动时先做一次；任何会改队列的操作持锁时也先做），排着的快照文件两边同名、原样留着。
 *
 * <p>不读会话，两个进程都能调；都会读写文件，放在工作线程上。
 */
public final class DictionarySnapshotQueue {
    /** 本地备份恢复排进来的请求的所有者，与 Rust 的 `LOCAL_RESTORE_OWNER` 相同：这份快照来自用户自己选的备份文件，不属于任何账号，认领和激活时不比对账号，退出登录也取消不到它。磁盘上已经排着的请求用的就是这个值，不能改。 */
    public static final String LOCAL_RESTORE_OWNER = "local-backup";
    /** 队列在应用私有 files 目录下的位置；两个进程用同一个绝对路径。 */
    public static final String PATH = "bootstrap/state/dictionary-snapshots";

    /** Rust 报的一次失败：snapshot_busy（已有一份在排队，或另一处正持着锁）、snapshot_conflict（本机词库版本已经变了）、snapshot_invalid、snapshot_unavailable 等。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        public final String code;

        Failure(String code) {
            super(code);
            this.code = code == null || code.isEmpty() ? "snapshot_unavailable" : code;
        }
    }

    private DictionarySnapshotQueue() {}

    /** 本应用的队列目录（绝对路径）；拿不到私有 files 目录时抛出。 */
    public static Path directory(Context context) throws IOException {
        File files = context.getApplicationContext().getFilesDir();
        if (files == null) throw new IOException("private files unavailable");
        return files.toPath().toAbsolutePath().normalize().resolve(PATH);
    }

    /**
     * 读出队列状态 `{version, localVersion, request}`（形状同 Rust 的 `SnapshotQueueState`）。经 `import_android_v1`：还没转换的旧队列这一步顺带在锁里转成 `state.json`；已经转换过（目录里没有 `state.bin`）、或目录不存在时什么也不改，也不创建目录，而且不取 `state.lock`，不会排在 `:ime` 正持锁的激活后面，`state.json` 由 Rust 不持锁地读。只读不会发布本机词库版本，`localVersion` 是键盘最后一次发布的。
     */
    public static JSONObject read(Path directory) throws Failure {
        Object state = importAndroidV1(directory).opt("state");
        if (!(state instanceof JSONObject object)) throw new Failure("snapshot_unavailable");
        return object;
    }

    /** 旧版 Java 队列的 `state.bin` 就地转成 `state.json`：`{imported, state}`。幂等，主进程启动时调一次。 */
    public static JSONObject importAndroidV1(Path directory) throws Failure {
        return call(request("import_android_v1", directory));
    }

    /** 键盘最后一次发布的本机词库版本（`local-v1:<代次>:<摘要>`）；还没发布过时为 null。 */
    public static String localVersion(JSONObject state) {
        return JsonPolicy.strictString(state.opt("localVersion"));
    }

    /**
     * 排一份快照，键盘下次没有会话时整份激活。Rust 把 `source` 复制进队列（不跟随符号链接、最多 512 MiB、核对 SHA-256），只在没有正在排队的请求、且 `expectedLocalVersion` 仍是键盘发布的版本时才收下，否则是 snapshot_busy 或 snapshot_conflict。返回排队以后的状态。
     *
     * @param owner 请求的所有者：云同步是账号 id，本地备份是 {@link #LOCAL_RESTORE_OWNER}
     */
    public static JSONObject enqueue(Path directory, Path source, String owner, long cloudRevision,
            String expectedLocalVersion, String fileSha256) throws Failure {
        try {
            return call(request("enqueue", directory)
                .put("source", source.toAbsolutePath().toString())
                .put("account_id", owner)
                .put("cloud_revision", cloudRevision)
                .put("expected_local_version", expectedLocalVersion)
                .put("file_sha256", fileSha256));
        } catch (JSONException invalid) {
            throw new Failure("snapshot_invalid");
        }
    }

    /** 取消 `accountId` 还在排队或准备中的请求（退出登录、换账号），等正在持锁的激活做完。返回取消以后的状态。 */
    public static JSONObject cancel(Path directory, String accountId) throws Failure {
        try {
            return call(request("cancel", directory).put("account_id", accountId));
        } catch (JSONException invalid) {
            throw new Failure("snapshot_invalid");
        }
    }

    /**
     * `:ime` 在键盘没有会话时调：发布本机词库版本，认领排着的请求，在 `stagingDirectory` 里准备好再激活。
     *
     * @param accountId 当前账号；null 表示不知道（provider 不可达），属于账号的请求原样留到下次，本地备份的请求照样激活；空串表示没有登录，属于账号的请求取消
     */
    public static JSONObject process(Path directory, Path stagingDirectory, String options, String accountId)
            throws Failure {
        try {
            JSONObject request = request("process", directory)
                .put("staging_root", stagingDirectory.toAbsolutePath().normalize().toString())
                .put("options", new JSONObject(options));
            if (accountId != null) request.put("account_id", accountId);
            return call(request);
        } catch (JSONException invalid) {
            throw new Failure("snapshot_invalid");
        }
    }

    /** 队列里有一个还在排队或准备中的请求。 */
    public static boolean active(JSONObject state) {
        JSONObject request = state.optJSONObject("request");
        String status = request == null ? null : JsonPolicy.strictString(request.opt("status"));
        return "queued".equals(status) || "preparing".equals(status);
    }

    /** 排着的请求不属于任何账号（本地备份恢复的）。 */
    public static boolean localRestore(JSONObject state) {
        JSONObject request = state.optJSONObject("request");
        return request != null && LOCAL_RESTORE_OWNER.equals(JsonPolicy.strictString(request.opt("accountId")));
    }

    private static JSONObject request(String operation, Path directory) throws Failure {
        if (directory == null || !directory.isAbsolute()) throw new Failure("snapshot_invalid");
        try {
            return new JSONObject().put("operation", operation).put("directory", directory.toString());
        } catch (JSONException invalid) {
            throw new Failure("snapshot_invalid");
        }
    }

    /** 信封 `{ok, value}` 或 `{ok:false, error}`；读不懂时按 snapshot_unavailable。 */
    private static JSONObject call(JSONObject request) throws Failure {
        final JSONObject root;
        try {
            root = new JSONObject(NativeClient.snapshotQueue(request.toString()));
        } catch (JSONException | RuntimeException malformed) {
            throw new Failure("snapshot_unavailable");
        }
        if (!JsonPolicy.strictTrue(root.opt("ok"))) throw new Failure(JsonPolicy.strictString(root.opt("error")));
        JSONObject value = JsonPolicy.objectValue(root);
        if (value == null) throw new Failure("snapshot_unavailable");
        return value;
    }
}
