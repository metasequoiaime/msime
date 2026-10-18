package app.msime.android;

import java.nio.file.Path;
import java.util.function.Supplier;
import org.json.JSONException;
import org.json.JSONObject;

/** 键盘会话结束以后才准备和激活词库快照；认领、准备和激活都在 Rust 的快照队列里（{@link DictionarySnapshotQueue#process}，#8201 迁移计划 5d）。 */
public final class DictionarySnapshotWorker {
    private DictionarySnapshotWorker() {}

    /** 先处理排着的整份快照激活，再合并本地备份恢复时排下的输入记录。合并会改本机词库版本，放在激活前面会让排着的激活因版本不符被拒。 */
    public static void process(Path queueDirectory, Path stagingDirectory, String options,
            Supplier<String> currentAccountId) throws Exception {
        try {
            activate(queueDirectory, stagingDirectory, options, currentAccountId);
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

    /**
     * 每次都交给 Rust 处理一次：它先发布本机词库版本（主进程排快照时要比对它），再认领、准备、激活排着的请求。
     *
     * <p>当前账号要经 provider 问主进程（一次 binder 调用），所以只在排着的请求属于某个账号时才问；没有请求、或者是本地备份恢复的请求（不属于任何账号）时不问，按「不知道」交给 Rust。读状态和处理之间主进程刚排进来的请求这一次按「不知道」原样留着，下次空闲再处理。
     */
    private static void activate(Path queueDirectory, Path stagingDirectory, String options,
            Supplier<String> currentAccountId) throws Exception {
        ensureSafeDirectory(stagingDirectory);
        JSONObject state = DictionarySnapshotQueue.read(queueDirectory);
        String accountId = DictionarySnapshotQueue.active(state) && !DictionarySnapshotQueue.localRestore(state)
            ? currentAccountId.get() : null;
        DictionarySnapshotQueue.process(queueDirectory, stagingDirectory, options, accountId);
    }

    static void ensureSafeDirectory(Path directory) throws java.io.IOException {
        if (directory == null) throw new java.io.IOException("snapshot staging directory unavailable");
        SafePaths.ensureDirectory(directory);
    }
}
