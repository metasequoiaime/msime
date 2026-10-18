package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import android.util.Log;
import app.msime.android.AccountStore;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.CloudSyncChoicePolicy;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.DigestPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.NativeClient;
import app.msime.android.RustCloudSync;
import app.msime.android.SyncSignals;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.io.IOException;
import java.lang.ref.WeakReference;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 宿主进程的云同步控制器：设置（含自定义键盘皮肤库）、无编码常用语、个人词库快照。
 *
 * <p>编排都在 Rust 的 `msime_client_cloud_sync`（#8201 迁移计划 5d，{@link RustCloudSync}）：只在登录账号且用户打开了同步开关时运行，登录本身不触发；节流（距上次满 5 分钟，或有分类带着待上传标记且满 30 秒）、Retry-After 退避、首次选择、三方合并、词库快照的下载与激活排队、状态行的措辞都由 Rust 决定。{@link HomeActivity} 每次回到前台调用 {@link #onResume}，用户刚打开开关时调用 {@link #runNow}。某个分类第一次同步而云端已有数据、或者云端被清空时，Rust 回答 needs_choice，这里弹出选择（{@link CloudSyncChoicePolicy}），用户选了再以 choice 跑一轮。
 *
 * <p>失败静默，下次回到前台再试；最近一次的失败原因在 Rust 的状态行里（{@link #status}）。命名词库的名字与启用状态不同步，只同步词条（{@link #SCOPE_NOTE}）。
 *
 * <p>每一轮都排在同一条工作线程上，一次只跑一轮；回到前台排着的那一轮还没轮到时不再多排一轮，用户的选择和刚打开开关的那一轮总会排上（另一轮还在跑时选的不会丢）。
 */
public final class CloudSync {
    /** 同步开关副标题里要说明的范围。 */
    public static final String SCOPE_NOTE = "同步设置、常用语、自定义皮肤和词库词条；命名词库的名字与启用状态不同步";

    private static final String TAG = "MSIMECloudSync";

    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor();
    /** 回到前台的那一轮已经排上、还没跑完。 */
    private static final AtomicBoolean ACTIVE_QUEUED = new AtomicBoolean();

    private CloudSync() {}

    /** 回到前台时调用，Rust 按节流决定要不要跑。 */
    public static void onResume(Activity activity) {
        if (!ACTIVE_QUEUED.compareAndSet(false, true)) return;
        request(activity, "active", null, () -> ACTIVE_QUEUED.set(false));
    }

    /** 用户刚打开同步开关时调用：不等节流，马上比对一次（首次需要选择时会弹出选择）。 */
    public static void runNow(Activity activity) {
        request(activity, "enable", null, null);
    }

    /** 「我的」页同步行用的状态：开关和 Rust 写好的那一句（失败原因、云端被清空、皮肤库被裁剪、收不下的常用语，或「已同步 · 最近 …」）。读状态失败时退回只看开关、不带那一句。阻塞，在工作线程上调用。 */
    static RustCloudSync.Status status(Context context) {
        try {
            return RustCloudSync.status(context);
        } catch (AccountStore.Failure unavailable) {
            Log.w(TAG, "cloud sync status unavailable: " + unavailable.code);
            return new RustCloudSync.Status(SyncSignals.enabled(context), "");
        }
    }

    /** 打开或关闭同步，返回 Rust 对齐以后的状态；没有登录的账号时打不开（cloud_sync_signed_out）。阻塞，在工作线程上调用。 */
    static RustCloudSync.Status setEnabled(Context context, boolean enabled) throws AccountStore.Failure {
        return RustCloudSync.setEnabled(context, enabled);
    }

    private static void request(Activity activity, String trigger, String choice, Runnable finished) {
        Context application = activity.getApplicationContext();
        WeakReference<Activity> owner = new WeakReference<>(activity);
        WORKER.execute(() -> {
            try {
                RustCloudSync.Round round = round(application, trigger, choice);
                if (round != null && round.needsChoice()) ask(owner, round.cloudCleared());
            } finally {
                if (finished != null) finished.run();
            }
        });
    }

    /** 跑一轮；回到前台而同步关着时不读本机就跳过（Rust 也会跳过，但要先交出整个皮肤库）。 */
    private static RustCloudSync.Round round(Context context, String trigger, String choice) {
        if (!HostStore.prepared(context)) return null;
        if ("active".equals(trigger) && !SyncSignals.enabled(context)) return null;
        try {
            return RustCloudSync.run(context, trigger, choice, HostStore.directory(context), hostOptions(context));
        } catch (AccountStore.Failure failure) {
            Log.w(TAG, "cloud sync round failed: " + failure.code);
        } catch (IOException | JSONException | RuntimeException error) {
            // 这是后台工作线程的边界：读不出本机值或原生库不可用时这一轮作废，下次回到前台再试，不能让线程带着异常退出、拖垮应用。
            Log.w(TAG, "cloud sync round failed", error);
        }
        return null;
    }

    /** 要用户在「合并 / 使用云端」（云端被清空时还有「关闭同步」）里选一个；取消就什么也不做，Rust 过一阵回到前台再问。 */
    private static void ask(WeakReference<Activity> owner, boolean cloudCleared) {
        Activity activity = owner.get();
        if (activity == null) return;
        CloudSyncChoicePolicy.Prompt prompt = CloudSyncChoicePolicy.prompt(cloudCleared);
        activity.runOnUiThread(() -> {
            Activity current = owner.get();
            if (current == null || current.isFinishing() || current.isDestroyed()) return;
            OptionSheet sheet = new OptionSheet(current, prompt.title(), prompt.message());
            for (CloudSyncChoicePolicy.Option option : prompt.options()) {
                sheet.option(option.label(), false, () -> request(current, "choice", option.choice(), null));
            }
            sheet.show();
        });
    }

    // ---- 设置文档（云同步和本地备份共用） ----

    /** 把一份设置文档（`{revision, settings}`）应用到本机：偏好由 client-core 按修订号保存，按键反馈、皮肤库和 Android 本地设置由这里写回各自的存储。`schema` 是服务端的字段表；本地备份恢复时是按备份里的值声明的字段表（{@link LocalBackup}）。 */
    static void applySettings(Context context, String directory, JSONObject document, JSONObject schema)
            throws IOException, JSONException {
        JSONObject request = new JSONObject()
            .put("preferences_directory", directory)
            .put("cloud", document)
            .put("schema", schema)
            .put("feedback", hostFeedback(context));
        JSONObject value = nativeValue(NativeClient.accountSettingsApply(request.toString()));
        JSONObject applied = value.optJSONObject("feedback");
        if (applied != null) {
            KeyboardFeedbackStore.save(context, KeyboardFeedbackStore.fromValues(
                applied.opt("soundEnabled"), applied.opt("hapticsEnabled"),
                applied.opt("hapticStrength")));
        }
        if (value.opt("custom_keyboard_skins") instanceof String library) {
            CustomSkinLibrary.importDesigns(Paths.get(directory), library);
        }
        JSONObject local = value.optJSONObject("android_local");
        if (local != null) AndroidLocalSettings.applySynced(context, map(local));
        JSONArray skipped = value.optJSONArray("skipped");
        if (skipped != null && skipped.length() > 0) Log.i(TAG, "settings skipped on this device: " + skipped);
    }

    /** 宿主当前的按键反馈，设置文档的导出和应用都要带上它。 */
    static JSONObject hostFeedback(Context context) throws JSONException {
        return RustCloudSync.feedback(context);
    }

    // ---- 个人词库（云同步和本地备份共用） ----

    /** 本机的宿主选项（`runtime-options.json`），词库操作都要带上它。 */
    static String hostOptions(Context context) throws IOException {
        File files = context.getFilesDir();
        if (files == null) throw new IOException("private files unavailable");
        return HostOptionsPolicy.read(new File(files, "runtime-options.json"));
    }

    /**
     * 把本机个人词库写成云端快照格式的 NDJSON（`export_snapshot`），返回快照的元数据（`entries` 是词数）：云同步上传和本地备份（{@link LocalBackup}）都用它。
     *
     * @param includeLearning 为真时再写上输入记录（学习调权、删除记录、固定位置和选词计数），元数据里多出 `learning` 条数。只有本地备份传真；为假时请求里不带这个字段，与云同步原来上传的快照逐字节相同。
     */
    static JSONObject exportDictionarySnapshot(Context context, Path destination, boolean includeLearning)
            throws IOException, JSONException {
        Files.deleteIfExists(destination);
        JSONObject action = new JSONObject().put("operation", "export_snapshot")
            .put("destination", destination.toAbsolutePath().toString());
        if (includeLearning) action.put("include_learning", true);
        return nativeValue(NativeClient.dictionary(new JSONObject()
            .put("options", new JSONObject(hostOptions(context)))
            .put("action", action).toString()));
    }

    /** 本机用户词库里的词数（不含内置词库）。 */
    static int userWordCount(Context context) throws IOException, JSONException {
        JSONObject value = nativeValue(NativeClient.dictionary(new JSONObject()
            .put("options", new JSONObject(hostOptions(context)))
            .put("action", new JSONObject().put("operation", "count").put("user_only", true)).toString()));
        Integer count = JsonPolicy.nonNegativeInteger(value.opt("count"));
        return count == null ? 0 : count;
    }

    /** 把一份词库快照交给激活队列（Rust 的 `msime_client_snapshot_queue`），键盘下次没有会话时用它整份替换用户词库。键盘还没发布过本机词库版本时抛 IOException；已有一份在排队（snapshot_busy）或本机词库版本已经变了（snapshot_conflict）时抛 {@link DictionarySnapshotQueue.Failure}。`owner` 是请求的来源：本地备份是 {@link DictionarySnapshotQueue#LOCAL_RESTORE_OWNER}（云同步的下载由 Rust 自己排）。 */
    static void enqueueDictionarySnapshot(Context context, Path file, String owner, long revision)
            throws IOException, DictionarySnapshotQueue.Failure {
        Path queue = DictionarySnapshotQueue.directory(context);
        String localVersion = DictionarySnapshotQueue.localVersion(DictionarySnapshotQueue.read(queue));
        if (localVersion == null) throw new IOException("keyboard has not published a dictionary version yet");
        DictionarySnapshotQueue.enqueue(queue, file.toAbsolutePath(), owner, revision, localVersion,
            DigestPolicy.sha256Hex(file));
    }

    // ---- JSON 小工具 ----

    /** client-core 的标准响应 `{ok, value, error}`：失败时抛出，信息只进日志。 */
    static JSONObject nativeValue(String response) throws JSONException {
        JSONObject root = new JSONObject(response == null ? "" : response);
        if (!JsonPolicy.strictTrue(root.opt("ok")))
            throw new IllegalStateException(root.optString("error", "native call failed"));
        JSONObject value = JsonPolicy.objectValue(root);
        return value == null ? new JSONObject() : value;
    }

    static Map<String, Object> map(JSONObject settings) {
        LinkedHashMap<String, Object> result = new LinkedHashMap<>(settings.length());
        Iterator<String> keys = settings.keys();
        while (keys.hasNext()) {
            String key = keys.next();
            Object value = settings.opt(key);
            if (value instanceof String || value instanceof Boolean || value instanceof Number) result.put(key, value);
        }
        return result;
    }


}
