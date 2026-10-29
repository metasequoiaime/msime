package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import java.io.File;
import java.io.IOException;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 本机数据有改动时通知云同步、在任何进程里看同步开关，以及读主进程记着的账号绑定。
 *
 * <p>同步状态在 Rust 的 `cloud-sync/state.json` 里（#8201 迁移计划 5d；每个进程第一次用到时先导入旧版本的进度，见 {@link RustCloudSync#migrate}），状态目录是 {@link #stateDirectory}，主进程和 `:ime` 用同一个绝对路径、经 flock 互斥。记改动是 Rust 的 `mark_dirty`，看开关是 `peek`：两者都只读写状态文件、不读会话，所以两个进程都直接调 Rust，不再经 {@link AccountSessionProvider} 绕回主进程。只有持有登录的主进程能调会按会话对齐的那几个操作（{@link RustCloudSync}），`:ime` 调了会被当成已经退出、把共享的进度清空。
 *
 * <p>账号绑定（账号编号与登录代次）仍是主进程的 {@link SyncSwitch}，`:ime` 经 provider 的 `sync_state` 读，回复里没有令牌。
 */
public final class SyncSignals {
    /** 同步状态目录在应用私有 files 目录下的位置；状态在它下面的 `cloud-sync/`。 */
    static final String STATE_PATH = "bootstrap/state";

    private SyncSignals() {}

    /** 同步状态目录（绝对路径）；拿不到私有 files 目录时抛出。 */
    public static String stateDirectory(Context context) throws IOException {
        File files = context.getApplicationContext().getFilesDir();
        if (files == null) throw new IOException("private files unavailable");
        return files.toPath().toAbsolutePath().normalize().resolve(STATE_PATH).toString();
    }

    /**
     * 标记一个分类（{@link SyncSwitch#SECTIONS}）本机有改动。同步关闭时 Rust 什么也不记。只读写状态文件、不联网，任何进程、任何线程都能调，但要等状态文件的锁，不要放在按键路径上。同步自己写本机不经过这里。
     *
     * @return 标记是否已经记下；同步关着或读写失败时返回 false，调用方不需要重试，下次打开同步时会整份比对
     */
    public static boolean markDirty(Context context, String section) {
        if (!SyncSwitch.validSection(section)) throw new IllegalArgumentException("unknown sync section");
        RustCloudSync.migrate(context);
        try {
            return markDirty(stateDirectory(context), section);
        } catch (IOException unavailable) {
            return false;
        }
    }

    /** {@link #markDirty(Context, String)} 在给定的状态目录上做；设备冒烟用它对着临时目录核对。 */
    static boolean markDirty(String stateDirectory, String section) {
        try {
            JSONObject value = call(stateDirectory, new JSONObject().put("operation", "mark_dirty").put("section", section), null);
            return value != null && JsonPolicy.strictTrue(value.opt("marked"));
        } catch (JSONException unavailable) {
            return false;
        }
    }

    /**
     * 本机清除了学习记录以后调（Rust 的 `dictionary_reset`）：取消清除之前下载、还在激活队列里等着的云端快照，忘掉等激活的记录，再给「个人词库」打待上传标记。不忘掉的话那份快照因本机版本变了而激活不了，下一轮会按合并把云端清除前的词并回本机。只读写状态文件和快照队列、不联网，放在工作线程上。
     *
     * @return 标记是否已经记下；同步关着或读写失败时返回 false
     */
    public static boolean dictionaryReset(Context context) {
        RustCloudSync.migrate(context);
        try {
            JSONObject extra = new JSONObject()
                .put("snapshot_queue_directory", DictionarySnapshotQueue.directory(context).toString());
            JSONObject value = call(stateDirectory(context), new JSONObject().put("operation", "dictionary_reset"), extra);
            return value != null && JsonPolicy.strictTrue(value.opt("marked"));
        } catch (IOException | JSONException unavailable) {
            return false;
        }
    }

    /** 同步开关现在开着没有（Rust 的 `peek`，不读会话、不对齐、不写）；读不到时按关闭处理。 */
    public static boolean enabled(Context context) {
        RustCloudSync.migrate(context);
        try {
            return enabled(stateDirectory(context));
        } catch (IOException unavailable) {
            return false;
        }
    }

    /** {@link #enabled(Context)} 在给定的状态目录上做。 */
    static boolean enabled(String stateDirectory) {
        try {
            JSONObject value = call(stateDirectory, new JSONObject().put("operation", "peek"), null);
            return value != null && JsonPolicy.strictTrue(value.opt("enabled"));
        } catch (JSONException unavailable) {
            return false;
        }
    }

    /** 当前主进程绑定的账号与代次；null 表示 provider 暂不可达或快照不一致。不要在按键路径调用。 */
    public static SyncSwitch.Binding binding(Context context) {
        Context application = context.getApplicationContext();
        if (AccountSessionRoutingPolicy.ownsSession(
                Application.getProcessName(), application.getPackageName())) {
            return SyncSwitch.bindingSnapshot(application);
        }
        try {
            Bundle reply = application.getContentResolver().call(providerUri(application),
                AccountSessionRoutingPolicy.METHOD_SYNC_STATE, null, null);
            if (reply == null || !reply.containsKey(AccountSessionRoutingPolicy.KEY_ACCOUNT_ID)
                    || !reply.containsKey(AccountSessionRoutingPolicy.KEY_BINDING_GENERATION)) return null;
            return new SyncSwitch.Binding(reply.getString(AccountSessionRoutingPolicy.KEY_ACCOUNT_ID),
                reply.getLong(AccountSessionRoutingPolicy.KEY_BINDING_GENERATION));
        } catch (RuntimeException unavailable) {
            return null;
        }
    }

    /** 当前主进程绑定的账号；null 表示 provider 暂不可达，空字符串表示账号已清除。 */
    public static String accountId(Context context) {
        SyncSwitch.Binding binding = binding(context);
        return binding == null ? null : binding.accountId();
    }

    /**
     * 跑一个不带会话来源的云同步操作（`peek`、`mark_dirty`、`dictionary_reset`、`import_legacy`），返回它的 value；Rust 报失败或回答读不懂时为 null。
     *
     * @param extra 请求里另外要带的字段（`import_legacy` 和 `dictionary_reset` 的 `snapshot_queue_directory`），没有时为 null
     */
    static JSONObject call(String stateDirectory, JSONObject action, JSONObject extra) throws JSONException {
        JSONObject request = extra == null ? new JSONObject() : new JSONObject(extra.toString());
        request.put("state_directory", stateDirectory).put("action", action);
        final JSONObject root;
        try {
            root = new JSONObject(NativeClient.cloudSync(request.toString()));
        } catch (JSONException | RuntimeException | LinkageError malformed) {
            return null;
        }
        return JsonPolicy.objectValue(root);
    }

    private static Uri providerUri(Context application) {
        return Uri.parse("content://" + AccountSessionRoutingPolicy.authority(application.getPackageName()));
    }
}
