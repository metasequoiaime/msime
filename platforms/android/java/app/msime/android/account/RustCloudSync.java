package app.msime.android;

import android.app.Application;
import android.content.Context;
import java.io.IOException;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 主进程经 Rust 的 {@code msime_client_cloud_sync} 跑自动云同步（#8201 迁移计划 5d）：设置（含自定义键盘皮肤库）、无编码常用语和个人词库的开关、节流、首次选择、三方合并、Retry-After 退避、词库快照的下载与激活排队、状态行的措辞都在 Rust 里。Java 只交出 Rust 读不到的本机值（按键反馈、Android 本地设置、皮肤库、还没认领的示例常用语），并在一轮回答 `pending_commit` 时先把 Rust 交回的值写进各自的存储、再提交。
 *
 * <p>请求带 {@link AccountStore} 注册的 `session_store`：status、set_enabled、run 每次都按会话里的用户 id 对齐，所以只在持有登录的主进程里调（{@link AccountStore#id} 在 `:ime` 里会抛出）。`:ime` 看开关、记改动用 {@link SyncSignals}。
 *
 * <p>升级以后第一次用到时先做两步迁移（{@link #migrate}），顺序不能反：先把 Java 旧快照队列的 `state.bin` 转成 Rust 的格式，再导入 `msime_sync_v1` 和 `msime_cloud_sync_status` 里的旧同步进度（带上快照队列目录）。反过来的话，旧版排着、还没激活的快照可能读不到，词库游标会被当成已经同步。
 *
 * <p>除 {@link #migrate} 的快照队列一步以外都要等状态文件的锁，run 还会联网（每个请求最长约 30 秒，词库快照最长 10 分钟），都在工作线程上调用。
 */
public final class RustCloudSync {
    /** 旧版本的同步进度。 */
    static final String LEGACY_STATUS = "msime_cloud_sync_status";

    private static final Object MIGRATION_LOCK = new Object();
    private static boolean migrated;

    /** 一轮的结果：`outcome` 是 Rust 的 skipped、needs_choice、done、pending_commit 或 disabled，`reason` 是它给的原因（needs_choice 时是 first_run 或 cloud_cleared）。 */
    public record Round(String outcome, String reason) {
        /** 要问用户「合并 / 使用云端」；云端被清空时多一个「关闭同步」。 */
        public boolean needsChoice() { return "needs_choice".equals(outcome); }

        public boolean cloudCleared() { return "cloud_cleared".equals(reason); }
    }

    /** 「我的」页同步行要的：开关，以及 Rust 写好的那一句（`status_line`）。 */
    public record Status(boolean enabled, String line) {}

    private RustCloudSync() {}

    /**
     * 升级以后的两步迁移，每个进程第一次用到云同步时做一次，顺序见类说明。两步都幂等：Rust 只在还没有任何进度、也没有绑定别的账号时导入旧进度，导入过一次、或者用户开关过同步以后不再导入，所以每次启动都带着旧状态调也不会把用户关掉的同步改回打开。快照队列那一步失败时照样导入同步进度：Rust 读不出队列就不导入词库游标，词库下一轮按首次同步处理。导入失败时下次再试。
     *
     * <p>`:ime` 也要导入：升级以后用户多半先用键盘、不打开应用，这时键盘记的改动（{@link SyncSignals#markDirty}）落在还没导入的全新状态上，开关是关的，Rust 不记，导入以后这次改动就不算待上传，云端一变还会被下载盖回去。`:ime` 只做导入这一步：转换旧队列要在 `state.lock` 上等正在进行的激活做完，而键盘可能在主线程上记改动；还没转换的旧队列 Rust 照样能读（导入时直接解码 `state.bin`），自己空闲时激活快照之前也会先转换。
     */
    public static void migrate(Context context) {
        synchronized (MIGRATION_LOCK) {
            if (migrated) return;
            Context application = context.getApplicationContext();
            Path queue;
            try {
                queue = DictionarySnapshotQueue.directory(application);
            } catch (IOException unavailable) {
                return;
            }
            if (AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), application.getPackageName())) {
                try {
                    DictionarySnapshotQueue.importAndroidV1(queue);
                } catch (DictionarySnapshotQueue.Failure unreadable) {
                    // 旧队列读不懂时 Rust 两份文件都不动；下面导入同步进度时也读不出它，词库游标不导入。
                }
            }
            try {
                migrated = importLegacy(SyncSignals.stateDirectory(application),
                    preferences(application, SyncSwitch.STORE), preferences(application, LEGACY_STATUS), queue);
            } catch (IOException unavailable) {
                // 拿不到私有目录时下次再试。
            }
        }
    }

    /**
     * 把旧版本两份 SharedPreferences 交给 Rust 的 `import_legacy`，返回 Rust 是否回答了（导没导入由 Rust 按自己的条件决定）。
     *
     * @param queue 词库快照激活队列的目录：队列里还有这个账号没应用的快照时，Rust 把它记成等激活的快照，第一轮不整份上传
     */
    static boolean importLegacy(String stateDirectory, JSONObject sync, JSONObject status, Path queue) {
        try {
            JSONObject action = new JSONObject().put("operation", "import_legacy")
                .put("android", new JSONObject().put("sync_v1", sync).put("cloud_sync_status", status));
            JSONObject extra = new JSONObject().put("snapshot_queue_directory", queue.toString());
            return SyncSignals.call(stateDirectory, action, extra) != null;
        } catch (JSONException unavailable) {
            return false;
        }
    }

    /** 一份 SharedPreferences 的全部键值写成 JSON：布尔、字符串、数字原样，字符串集合（`phrases_unheld`）是数组。 */
    static JSONObject preferences(Context context, String name) {
        return preferences(context.getSharedPreferences(name, Context.MODE_PRIVATE).getAll());
    }

    static JSONObject preferences(Map<String, ?> values) {
        JSONObject result = new JSONObject();
        for (Map.Entry<String, ?> entry : values.entrySet()) {
            Object value = entry.getValue();
            try {
                if (value instanceof Set<?> set) {
                    JSONArray texts = new JSONArray();
                    for (Object text : set) if (text instanceof String string) texts.put(string);
                    result.put(entry.getKey(), texts);
                } else if (value instanceof Boolean || value instanceof String || value instanceof Number) {
                    result.put(entry.getKey(), value);
                }
            } catch (JSONException skipped) {
                // 非有限的浮点数放不进 JSON；旧进度里没有这种值，跳过它。
            }
        }
        return result;
    }

    /** 与当前登录对齐后的状态：换了账号或已经退出时，Rust 在这一步关掉开关、清空进度。会话读不出来时不对齐。 */
    public static Status status(Context context) throws AccountStore.Failure {
        migrate(context);
        return statusOf(call(context, null, new JSONObject(), "status"));
    }

    /** 打开或关闭同步。打开时 Rust 把全部分类标成待上传，没有登录的账号时是 cloud_sync_signed_out；关闭时保留游标。 */
    public static Status setEnabled(Context context, boolean enabled) throws AccountStore.Failure {
        migrate(context);
        try {
            return statusOf(call(context, null, new JSONObject().put("enabled", enabled), "set_enabled"));
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("cloud_sync_invalid", 0);
        }
    }

    /**
     * 退出登录、换账号之前调：Rust 清空进度、代次加一，进行中的那一轮因此提交不了，并取消旧账号排队中的词库快照。要在清掉本机会话之前调，请求带的会话来源那时还看得到这个账号。
     */
    public static void signOut(Context context) throws AccountStore.Failure {
        migrate(context);
        try {
            JSONObject fields = new JSONObject()
                .put("snapshot_queue_directory", DictionarySnapshotQueue.directory(context).toString());
            call(context, fields, new JSONObject(), "sign_out");
        } catch (IOException unavailable) {
            throw new AccountStore.Failure("cloud_sync_storage", 0);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("cloud_sync_invalid", 0);
        }
    }

    /**
     * 跑一轮。`pending_commit` 时先在账号绑定锁里核对登录的仍是这一轮的账号，把 `host_writes` 写进本机，再以 `run_id` 提交；写本机失败或账号变了时不提交，Rust 的租约 180 秒后过期，下一轮重新下载，结果相同。
     *
     * @param trigger active（回到前台，Rust 按节流决定跑不跑）、enable（刚打开开关）或 choice（用户选了一项）
     * @param choice merge、use_cloud 或 disable（只回答「云端被清空」）；trigger 不是 choice 时为 null
     * @param preferencesDirectory 共享偏好和常用语所在的偏好目录
     * @param options Engine 的宿主选项（`runtime-options.json`），同步词库要用
     */
    public static Round run(Context context, String trigger, String choice, String preferencesDirectory,
            String options) throws AccountStore.Failure, IOException, JSONException {
        migrate(context);
        JSONObject inputs = new JSONObject()
            .put("feedback", feedback(context))
            .put("android_local", new JSONObject(AndroidLocalSettings.load(context).synced()))
            .put("custom_keyboard_skins", CustomSkinLibrary.exportDesigns(Paths.get(preferencesDirectory)))
            .put("excluded_phrase_texts", new JSONArray(CommonPhrasesStore.untouchedStarters(context)));
        JSONObject action = new JSONObject().put("trigger", trigger)
            .put("capabilities", new JSONObject().put("settings", true).put("skins_library", true)
                .put("phrases", true).put("dictionary", true))
            .put("host_inputs", inputs);
        if (choice != null) action.put("choice", choice);
        JSONObject fields = new JSONObject().put("preferences_directory", preferencesDirectory)
            .put("options", new JSONObject(options))
            .put("snapshot_queue_directory", DictionarySnapshotQueue.directory(context).toString());
        JSONObject result = object(call(context, fields, action, "run"));
        String outcome = JsonPolicy.strictStringOrEmpty(result.opt("outcome"));
        String reason = JsonPolicy.strictStringOrEmpty(result.opt("reason"));
        if ("pending_commit".equals(outcome)) {
            commit(context, preferencesDirectory, result);
        }
        return new Round(outcome, reason);
    }

    private static void commit(Context context, String preferencesDirectory, JSONObject result)
            throws AccountStore.Failure, IOException, JSONException {
        String runId = JsonPolicy.strictString(result.opt("run_id"));
        JSONObject status = result.optJSONObject("status");
        String account = status == null ? null : JsonPolicy.strictString(status.opt("account_id"));
        if (runId == null || account == null) return;
        JSONObject writes = result.optJSONObject("host_writes");
        synchronized (SyncSwitch.bindingLock()) {
            // 联网的那段时间里退出或换了号：这一轮下载的是旧账号的值，不写进本机。
            if (!account.equals(RustAccount.userId(context))) return;
            if (writes != null) write(context, preferencesDirectory, writes);
            JSONObject fields = new JSONObject().put("preferences_directory", preferencesDirectory);
            call(context, fields, new JSONObject().put("run_id", runId), "commit");
        }
    }

    /** 把 Rust 交回的本机值写进各自的存储。这些都是同步自己写的，不记成本机改动。 */
    static void write(Context context, String preferencesDirectory, JSONObject writes)
            throws IOException, JSONException {
        JSONObject feedback = writes.optJSONObject("feedback");
        if (feedback != null) {
            KeyboardFeedbackStore.save(context, KeyboardFeedbackStore.fromValues(
                feedback.opt("soundEnabled"), feedback.opt("hapticsEnabled"), feedback.opt("hapticStrength")));
        }
        JSONObject local = writes.optJSONObject("android_local");
        if (local != null) AndroidLocalSettings.applySynced(context, scalars(local));
        if (writes.opt("custom_keyboard_skins") instanceof String library) {
            CustomSkinLibrary.importDesigns(Paths.get(preferencesDirectory), library);
        }
        JSONArray adopt = writes.optJSONArray("adopt_texts");
        if (adopt != null) CommonPhrasesStore.adoptStarters(context, texts(adopt));
    }

    /** 当前的按键反馈，设置文档的导出和应用都要带上它。 */
    public static JSONObject feedback(Context context) throws JSONException {
        KeyboardFeedbackStore.Settings settings = KeyboardFeedbackStore.load(context);
        return new JSONObject()
            .put("soundEnabled", settings.soundEnabled())
            .put("hapticsEnabled", settings.hapticsEnabled())
            .put("hapticStrength", settings.hapticStrength().id());
    }

    private static Status statusOf(Object value) throws AccountStore.Failure {
        JSONObject status = object(value);
        return new Status(JsonPolicy.strictTrue(status.opt("enabled")),
            JsonPolicy.strictStringOrEmpty(status.opt("status_line")));
    }

    private static JSONObject object(Object value) throws AccountStore.Failure {
        if (!(value instanceof JSONObject object)) throw new AccountStore.Failure("cloud_sync_storage", 0);
        return object;
    }

    private static Map<String, Object> scalars(JSONObject values) {
        Map<String, Object> result = new java.util.LinkedHashMap<>(values.length());
        java.util.Iterator<String> keys = values.keys();
        while (keys.hasNext()) {
            String key = keys.next();
            Object value = values.opt(key);
            if (value instanceof String || value instanceof Boolean || value instanceof Number) result.put(key, value);
        }
        return result;
    }

    private static Collection<String> texts(JSONArray values) {
        List<String> result = new ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) {
            if (values.opt(index) instanceof String text) result.add(text);
        }
        return result;
    }

    /** 带会话来源的一次调用，返回 value；失败时抛出 Rust 的错误码（cloud_sync_*、account_*）。 */
    private static Object call(Context context, JSONObject fields, JSONObject action, String operation)
            throws AccountStore.Failure {
        final String request;
        try {
            JSONObject body = fields == null ? new JSONObject() : new JSONObject(fields.toString());
            body.put("session_store", AccountStore.id(context))
                .put("state_directory", SyncSignals.stateDirectory(context))
                .put("action", new JSONObject(action.toString()).put("operation", operation));
            request = body.toString();
        } catch (IOException unavailable) {
            throw new AccountStore.Failure("cloud_sync_storage", 0);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("cloud_sync_invalid", 0);
        }
        return AccountStore.value(NativeClient.cloudSync(request));
    }
}
