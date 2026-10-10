package app.msime.android.home;

import app.msime.android.TextPolicy;
import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;
import android.util.Log;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.BoundsPolicy;
import app.msime.android.CloudApi;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.DigestPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.NativeClient;
import app.msime.android.SafePaths;
import app.msime.android.SyncApi;
import app.msime.android.SyncMergePolicy;
import app.msime.android.SyncSwitch;
import app.msime.android.TextPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.io.IOException;
import java.lang.ref.WeakReference;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 宿主进程的云同步控制器：设置（含自定义键盘皮肤库）、无编码常用语、个人词库快照。
 *
 * <p>只在真实账号（{@link SyncSwitch#LOGIN_KINDS}）且用户打开了同步开关时运行，登录本身不触发。{@link HomeActivity} 每次回到前台调用 {@link #onResume}，距上次尝试满 5 分钟、或有分类带着待上传标记且满 30 秒时才真正跑（{@link SyncMergePolicy#due}）。某个分类第一次同步而云端已有数据时，先弹「合并 / 使用云端」让用户选，选了才执行。
 *
 * <p>失败静默，下次回到前台再试；最近一次的失败原因只留一行给「我的」页的同步行副标题（{@link #statusLine}）。命名词库的名字与启用状态不同步，只同步词条（{@link #SCOPE_NOTE}）。隐私模式、开发者选项、诊断日志与语音数据贡献在 client-core 的导出里已经排除，这里上传前和应用前再各过滤一次（{@link SyncMergePolicy#withoutLocalOnly}）。
 */
public final class CloudSync {
    /** 同步开关副标题里要说明的范围。 */
    public static final String SCOPE_NOTE = "同步设置、常用语、自定义皮肤和词库词条；命名词库的名字与启用状态不同步";

    private static final String TAG = "MSIMECloudSync";
    private static final String STATUS = "msime_cloud_sync_status";
    private static final String KEY_LAST_ATTEMPT = "last_attempt";
    private static final String KEY_PROMPTED_AT = "prompted_at";
    private static final String KEY_ERROR = "error";
    private static final String KEY_SKINS_TRIMMED = "skins_trimmed";
    private static final String KEY_STATUS_BINDING = "binding_generation";
    private static final String QUEUE_PATH = "bootstrap/state/dictionary-snapshots";
    private static final String WORK_PATH = "bootstrap/state/cloud-sync";

    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor();
    private static final AtomicBoolean RUNNING = new AtomicBoolean();

    private CloudSync() {}

    /** 回到前台时调用，按节流决定要不要跑。 */
    public static void onResume(Activity activity) {
        request(activity, null, false);
    }

    /** 用户刚打开同步开关时调用：不等节流，马上比对一次（首次需要选择时会弹出选择）。 */
    public static void runNow(Activity activity) {
        request(activity, null, true);
    }

    /** 「我的」页同步行副标题用的一行状态：最近一次失败的原因、皮肤库被裁剪的提示，或云端常用语本机收不下的提示；都没有时为空串。 */
    public static String statusLine(Context context) {
        SharedPreferences status = status(context);
        if (status.getLong(KEY_STATUS_BINDING, -1L) != SyncSwitch.bindingGeneration(context)) return "";
        String error = status.getString(KEY_ERROR, "");
        if (!error.isEmpty()) return error;
        if (status.getBoolean(KEY_SKINS_TRIMMED, false)) return "自定义皮肤太多，只同步了最近的设计";
        int unheld = SyncSwitch.unheldPhrases(context).size();
        return unheld > 0 ? "云端有 " + unheld + " 条常用语超出本机上限，未下载但已保留" : "";
    }

    private static void request(Activity activity, SyncMergePolicy.Choice choice, boolean force) {
        Context application = activity.getApplicationContext();
        WeakReference<Activity> owner = new WeakReference<>(activity);
        if (!RUNNING.compareAndSet(false, true)) return;
        WORKER.execute(() -> {
            try {
                run(application, owner, choice, force);
            } finally {
                RUNNING.set(false);
            }
        });
    }

    private static boolean eligible(Context context) {
        return SyncSwitch.enabled(context) && SyncSwitch.validLoginKind(SyncSwitch.loginKind(context))
            && HostStore.prepared(context);
    }

    private static void run(Context context, WeakReference<Activity> owner, SyncMergePolicy.Choice choice,
            boolean force) {
        if (!eligible(context)) return;
        String accountId = SyncSwitch.accountId(context);
        long bindingGeneration = SyncSwitch.bindingGeneration(context);
        boolean anyDirty = false;
        for (String section : SyncSwitch.SECTIONS) anyDirty |= SyncSwitch.dirty(context, section);
        long now = System.currentTimeMillis();
        SharedPreferences status = status(context);
        boolean sameBinding = status.getLong(KEY_STATUS_BINDING, -1L) == bindingGeneration;
        if (!force && choice == null && !SyncMergePolicy.due(now,
                sameBinding ? status.getLong(KEY_LAST_ATTEMPT, 0L) : 0L, anyDirty)) {
            return;
        }
        status.edit().putLong(KEY_LAST_ATTEMPT, now).putLong(KEY_STATUS_BINDING, bindingGeneration).apply();
        Session session = new Session(context, new SyncApi(context, bindingGeneration), choice,
            accountId, bindingGeneration);
        String failure;
        try {
            if (!session.probe()) {
                long promptedAt = sameBinding ? status.getLong(KEY_PROMPTED_AT, 0L) : 0L;
                if (!session.current()) return;
                if (force || promptedAt <= 0 || now < promptedAt || now - promptedAt >= SyncMergePolicy.THROTTLE_MILLIS) {
                    status.edit().putLong(KEY_PROMPTED_AT, now).apply();
                    askFirstRun(owner);
                }
                return;
            }
            failure = session.run();
        } catch (CloudApi.Failure error) {
            Log.w(TAG, "cloud sync probe failed: " + error.status + " " + error.code);
            failure = message(error);
        }
        // 跑的过程中退出登录或换了账号：结果作废，不写进度。
        synchronized (SyncSwitch.bindingLock()) {
            if (!session.current()) return;
            SharedPreferences.Editor editor = status.edit().putLong(KEY_STATUS_BINDING, bindingGeneration)
                .putBoolean(KEY_SKINS_TRIMMED, session.skinsTrimmed);
            if (failure.isEmpty()) {
                SyncSwitch.setLastSyncedAtIfCurrent(context, System.currentTimeMillis(), bindingGeneration);
                editor.remove(KEY_ERROR);
            } else {
                editor.putString(KEY_ERROR, failure);
            }
            editor.apply();
        }
    }

    /** 首次同步而云端已有数据：问用户「合并」还是「使用云端」；取消就什么也不做，过一阵回到前台再问。 */
    private static void askFirstRun(WeakReference<Activity> owner) {
        Activity activity = owner.get();
        if (activity == null) return;
        activity.runOnUiThread(() -> {
            Activity current = owner.get();
            if (current == null || current.isFinishing() || current.isDestroyed()) return;
            new OptionSheet(current, "云端已有同步数据", "合并会保留两边的内容，同一项以本机为准；使用云端会用云端的内容替换本机")
                .option("合并", false, () -> request(current, SyncMergePolicy.Choice.MERGE, true))
                .option("使用云端", false, () -> request(current, SyncMergePolicy.Choice.USE_CLOUD, true))
                .show();
        });
    }

    /**
     * 展示给用户的同步失败原因。
     *
     * <p>原先除了网络、登录和停用三种，其余一律说「同步失败，稍后自动重试」，但很多失败重试也不会好（云端不收某条词、词库超过上限），用户只能一直看着它失败，反馈过来也查不到原因：服务端日志不记用户和错误码。所以按 msime-cloud 返回的错误码说清楚是哪一种；不认识的带上状态码和错误码，方便用户截图反馈。
     */
    static String message(CloudApi.Failure failure) {
        if (failure.network()) return "网络不可用，稍后自动重试";
        if (failure.signedOut()) return "登录已失效，请重新登录后再同步";
        if (failure.unavailable()) return "云同步暂时不可用";
        switch (failure.code) {
            case "invalid_dictionary_snapshot", "invalid_dictionary_entry":
                return "词库里有云端暂不支持的词条，词库没有同步（" + failure.code + "）";
            case "dictionary_limit": return "云端词库最多保存 10 万条，词库没有同步";
            case "snapshot_too_large": return "词库太大，无法同步到云端";
            case "invalid_preference_field", "invalid_preferences": return "有设置项云端不认识，设置没有同步（" + failure.code + "）";
            case "invalid_phrases": return "常用语超出云端限制（最多 500 条，每条最多 2000 字），常用语没有同步";
            case "account_banned": return "账号已被停用，无法同步";
            case "rate_limit_exceeded": return "同步太频繁，稍后自动重试";
            case "snapshot_restore_busy", "engine_unavailable", "auth_unavailable", "dictionary_timeout":
                return "云端暂时繁忙，稍后自动重试";
            default: break;
        }
        if (failure.status == 429 || failure.status >= 500) return "云端暂时繁忙，稍后自动重试（HTTP " + failure.status + "）";
        return "同步失败（HTTP " + failure.status + (failure.code.isEmpty() ? "" : " " + failure.code) + "）";
    }

    /** 应用一次云端常用语的结果：成功写入本机的次数（每次都会把代数加一），以及本机收不下的正文。 */
    private record PhraseApply(int writes, Set<String> unheld) {}

    private static SharedPreferences status(Context context) {
        return context.getApplicationContext().getSharedPreferences(STATUS, Context.MODE_PRIVATE);
    }

    /** 一次同步：先把三类云端状态都读一遍，决定要不要问用户，再逐类执行；一类失败不挡其他类。 */
    private static final class Session {
        private final Context context;
        private final SyncApi api;
        private final SyncMergePolicy.Choice choice;
        private final String directory;
        private final String accountId;
        private final long bindingGeneration;
        private JSONObject schema;
        private SyncApi.Preferences preferences;
        private SyncApi.Phrases phrases;
        private SyncApi.DictionaryProbe dictionary;
        boolean skinsTrimmed;

        Session(Context context, SyncApi api, SyncMergePolicy.Choice choice,
                String accountId, long bindingGeneration) {
            this.context = context;
            this.api = api;
            this.choice = choice;
            this.directory = HostStore.directory(context);
            this.accountId = accountId;
            this.bindingGeneration = bindingGeneration;
        }

        boolean current() {
            return eligible(context) && accountId.equals(SyncSwitch.accountId(context))
                && bindingGeneration == SyncSwitch.bindingGeneration(context);
        }

        private boolean first(String section) {
            return SyncSwitch.cursor(context, section).isEmpty();
        }

        /** 读云端状态；返回 false 表示有分类第一次同步而云端已有数据，需要先问用户。 */
        boolean probe() throws CloudApi.Failure {
            if (!current()) return false;
            schema = api.preferencesSchema();
            preferences = api.preferences();
            phrases = api.phrases();
            dictionary = api.dictionaryChangedSince(first(SyncSwitch.DICTIONARY) ? 0L : cursorRevision(SyncSwitch.DICTIONARY));
            if (!current()) return false;
            if (choice != null) return true;
            boolean needs = first(SyncSwitch.SETTINGS) && !preferences.settings().isEmpty()
                || first(SyncSwitch.PHRASES) && !phrases.phrases().isEmpty()
                || first(SyncSwitch.DICTIONARY) && dictionary.changed();
            return !needs;
        }

        /** 逐类执行，返回第一条要展示的失败原因，全部成功时为空串。 */
        String run() {
            String failure = "";
            try {
                settings();
            } catch (CloudApi.Failure error) {
                Log.w(TAG, "settings sync failed: " + error.status + " " + error.code);
                failure = message(error);
            } catch (IOException | JSONException | IllegalStateException error) {
                Log.w(TAG, "settings sync failed", error);
                failure = "设置同步失败，稍后自动重试";
            }
            try {
                phrases();
            } catch (CloudApi.Failure error) {
                Log.w(TAG, "phrases sync failed: " + error.status + " " + error.code);
                if (failure.isEmpty()) failure = message(error);
            } catch (IllegalStateException error) {
                Log.w(TAG, "phrases sync failed", error);
                if (failure.isEmpty()) failure = "常用语同步失败，稍后自动重试";
            }
            try {
                dictionary();
            } catch (CloudApi.Failure error) {
                Log.w(TAG, "dictionary sync failed: " + error.status + " " + error.code);
                if (failure.isEmpty()) failure = message(error);
            } catch (IOException | JSONException | DictionarySnapshotQueue.Failure | IllegalStateException error) {
                Log.w(TAG, "dictionary sync failed", error);
                if (failure.isEmpty()) failure = "词库同步失败，稍后自动重试";
            }
            return failure;
        }

        private long cursorRevision(String section) {
            try {
                return BoundsPolicy.nonNegative(Long.parseLong(SyncSwitch.cursor(context, section)));
            } catch (NumberFormatException never) {
                return 0L;
            }
        }

        private SyncMergePolicy.Mode mode(String section, boolean cloudHasData, boolean cloudChanged, boolean dirty) {
            if (first(section)) {
                SyncMergePolicy.Mode mode = SyncMergePolicy.firstRun(cloudHasData, choice);
                return mode == null ? SyncMergePolicy.Mode.NONE : mode;
            }
            return SyncMergePolicy.incremental(cloudChanged, dirty);
        }

        // ---- 设置与皮肤库 ----

        private void settings() throws CloudApi.Failure, IOException, JSONException {
            if (!current()) return;
            // 先记下改动代数再读本机：上传期间用户又改了设置，代数会变大，标记留着，下一轮再传。
            long settingsGeneration = SyncSwitch.generation(context, SyncSwitch.SETTINGS);
            long skinsGeneration = SyncSwitch.generation(context, SyncSwitch.SKINS);
            String cursor = SyncSwitch.cursor(context, SyncSwitch.SETTINGS);
            boolean dirty = SyncSwitch.dirty(context, SyncSwitch.SETTINGS) || SyncSwitch.dirty(context, SyncSwitch.SKINS);
            SyncApi.Preferences cloud = preferences;
            SyncMergePolicy.Mode mode = mode(SyncSwitch.SETTINGS, !cloud.settings().isEmpty(),
                SyncMergePolicy.cloudChanged(cursor, cloud.revision()), dirty);
            SyncApi.Preferences result = cloud;
            for (int attempt = 0; ; attempt++) {
                if (mode == SyncMergePolicy.Mode.NONE) break;
                if (mode == SyncMergePolicy.Mode.DOWNLOAD) {
                    synchronized (SyncSwitch.bindingLock()) {
                        if (!current()) return;
                        apply(cloud);
                    }
                    result = cloud;
                    break;
                }
                // 合并时先把云端皮肤库并进本机（按 id 和更新时间），否则本机的库会整份盖掉云端的设计。
                if (mode == SyncMergePolicy.Mode.MERGE && cloud.settings().get(SyncMergePolicy.SKINS_KEY) instanceof String library) {
                    synchronized (SyncSwitch.bindingLock()) {
                        if (!current()) return;
                        CustomSkinLibrary.importDesigns(Paths.get(directory), library);
                    }
                }
                Map<String, Object> merged = exportMerged(cloud);
                if (!current()) return;
                try {
                    result = api.putPreferences(cloud.revision(), merged);
                } catch (CloudApi.Failure failure) {
                    if (!SyncApi.conflict(failure) || attempt >= SyncMergePolicy.MAX_CONFLICT_RETRIES) throw failure;
                    // 别的设备先写了：重新拉，以本机为后写合并后再试。
                    cloud = api.preferences();
                    mode = SyncMergePolicy.Mode.MERGE;
                    continue;
                }
                // 合并结果里可能有云端独有的键，应用回本机；与本机相同的部分 client-core 不会重写。
                synchronized (SyncSwitch.bindingLock()) {
                    if (!current()) return;
                    apply(result);
                }
                break;
            }
            if (!current()) return;
            SyncSwitch.setCursorIfCurrent(context, SyncSwitch.SETTINGS, Long.toString(result.revision()), bindingGeneration);
            SyncSwitch.setCursorIfCurrent(context, SyncSwitch.SKINS, Long.toString(result.revision()), bindingGeneration);
            SyncSwitch.clearDirtyIfCurrent(context, SyncSwitch.SETTINGS, settingsGeneration, bindingGeneration);
            SyncSwitch.clearDirtyIfCurrent(context, SyncSwitch.SKINS, skinsGeneration, bindingGeneration);
        }

        /** 本机设置叠加到云端文档上的整份结果；皮肤库按剩下的字节预算从最近的设计装起。 */
        private Map<String, Object> exportMerged(SyncApi.Preferences cloud) throws IOException, JSONException {
            JSONObject request = new JSONObject()
                .put("preferences_directory", directory)
                .put("feedback", feedback())
                .put("android_local", new JSONObject(AndroidLocalSettings.load(context).synced()))
                .put("schema", schema)
                .put("cloud", document(cloud.revision(), cloud.settings()));
            JSONObject withoutSkins = settingsOf(nativeValue(NativeClient.accountSettingsExport(request.toString()))
                .getJSONObject("merged"));
            withoutSkins.remove(SyncMergePolicy.SKINS_KEY);
            long otherBytes = TextPolicy.utf8Length(new JSONObject()
                .put("revision", cloud.revision()).put("settings", withoutSkins).toString());
            List<CustomSkinLibrary.Item> items = SyncMergePolicy.newestFirst(
                CustomSkinLibrary.read(Paths.get(directory)), CustomSkinLibrary.Item::updatedAt);
            String library = CustomSkinLibrary.exportDesigns(items, SyncMergePolicy.skinBudget(otherBytes));
            int kept = new JSONArray(library).length();
            skinsTrimmed = SyncMergePolicy.skinsTrimmed(items.size(), kept);
            if (skinsTrimmed) Log.i(TAG, "custom skin library trimmed to " + kept + " of " + items.size());
            request.put("custom_keyboard_skins", library);
            JSONObject merged = settingsOf(nativeValue(NativeClient.accountSettingsExport(request.toString()))
                .getJSONObject("merged"));
            return SyncMergePolicy.withoutLocalOnly(map(merged));
        }

        private void apply(SyncApi.Preferences cloud) throws IOException, JSONException {
            applySettings(context, directory,
                document(cloud.revision(), SyncMergePolicy.withoutLocalOnly(cloud.settings())), schema);
        }

        private JSONObject feedback() throws JSONException {
            return hostFeedback(context);
        }

        // ---- 常用语 ----

        private void phrases() throws CloudApi.Failure {
            if (!current()) return;
            long generation = SyncSwitch.generation(context, SyncSwitch.PHRASES);
            Map<String, String> local = ownPhrases();
            SyncApi.Phrases cloud = phrases;
            String cursor = SyncSwitch.cursor(context, SyncSwitch.PHRASES);
            SyncMergePolicy.Mode mode = mode(SyncSwitch.PHRASES, !cloud.phrases().isEmpty(),
                SyncMergePolicy.cloudChanged(cursor, cloud.revision()), SyncSwitch.dirty(context, SyncSwitch.PHRASES));
            long revision = cloud.revision();
            List<SyncMergePolicy.Phrase> target = null;
            for (int attempt = 0; ; attempt++) {
                if (mode == SyncMergePolicy.Mode.NONE) break;
                if (mode == SyncMergePolicy.Mode.DOWNLOAD) {
                    target = cloud.phrases();
                    break;
                }
                List<SyncMergePolicy.Phrase> upload = mode == SyncMergePolicy.Mode.MERGE
                    ? SyncMergePolicy.mergePhrases(asPhrases(local, cloud.phrases()), cloud.phrases())
                    : SyncMergePolicy.uploadPhrases(asPhrases(local, cloud.phrases()), cloud.phrases(),
                        SyncSwitch.unheldPhrases(context));
                try {
                    if (!current()) return;
                    SyncApi.Phrases saved = api.putPhrases(cloud.revision(), upload);
                    revision = saved.revision();
                    target = saved.phrases();
                } catch (CloudApi.Failure failure) {
                    if (!SyncApi.conflict(failure) || attempt >= SyncMergePolicy.MAX_CONFLICT_RETRIES) throw failure;
                    cloud = api.phrases();
                    mode = SyncMergePolicy.Mode.MERGE;
                    continue;
                }
                break;
            }
            int ownWrites = 0;
            if (target != null) {
                synchronized (SyncSwitch.bindingLock()) {
                    if (!current()) return;
                    adoptStarters(target);
                    PhraseApply applied = applyPhrases(local, target);
                    ownWrites = applied.writes();
                    SyncSwitch.setUnheldPhrasesIfCurrent(context, applied.unheld(), bindingGeneration);
                }
            }
            if (!current()) return;
            SyncSwitch.setCursorIfCurrent(context, SyncSwitch.PHRASES, Long.toString(revision), bindingGeneration);
            // 本机写入也会经 CommonPhrasesStore 把代数加一，每次成功写入一次；只在代数恰好是「读快照前 + 自己的写入」时清标记，期间用户的改动留到下一轮上传。
            SyncSwitch.clearDirtyIfCurrent(context, SyncSwitch.PHRASES, generation + ownWrites, bindingGeneration);
        }

        /**
         * 用户自己添加的常用语（id → 正文），按本机顺序；社区短语包里的不同步，装包的设备各自管理。
         *
         * <p>本机预置、还没被认领的示例也跳过：它们不上传，合并时也就不会出现在用户已经删掉它们的别的设备上；不在这份列表里，下载和「使用云端」也不会把它们当成本机多出来的删掉。
         */
        private Map<String, String> ownPhrases() {
            CommonPhrasesStore.Result result = CommonPhrasesStore.load(context);
            if (!result.ok()) throw new IllegalStateException("common phrases unavailable: " + result.failure());
            Set<String> starters;
            try {
                starters = CommonPhrasesStore.untouchedStarters(context);
            } catch (IOException error) {
                throw new IllegalStateException("starter phrase record unavailable", error);
            }
            LinkedHashMap<String, String> own = new LinkedHashMap<>(result.document().phrases().size());
            for (CommonPhrasesStore.Phrase phrase : result.document().phrases()) {
                if (phrase.own() && !starters.contains(phrase.text())) own.put(phrase.id(), phrase.text());
            }
            return own;
        }

        /** 云端或合并结果里已有的正文即使和本机的示例相同，也是用户的常用语了：先认领，免得之后只有本机改动的上传把它从云端删掉。写不下认领记录就放弃这一轮，游标不前进，下一轮重来。 */
        private void adoptStarters(List<SyncMergePolicy.Phrase> target) {
            List<String> texts = new ArrayList<>(target.size());
            for (SyncMergePolicy.Phrase phrase : target) texts.add(phrase.text());
            try {
                CommonPhrasesStore.adoptStarters(context, texts);
            } catch (IOException error) {
                throw new IllegalStateException("starter phrase record unavailable", error);
            }
        }

        /** 本机列表换成云端格式；同一正文在云端有分组时沿用云端的分组。 */
        private List<SyncMergePolicy.Phrase> asPhrases(Map<String, String> local, List<SyncMergePolicy.Phrase> cloud) {
            HashMap<String, String> groups = new HashMap<>(cloud.size());
            for (SyncMergePolicy.Phrase phrase : cloud) groups.putIfAbsent(phrase.text(), phrase.group());
            List<SyncMergePolicy.Phrase> result = new ArrayList<>(local.size());
            for (Map.Entry<String, String> entry : local.entrySet()) {
                result.add(new SyncMergePolicy.Phrase(entry.getKey(), entry.getValue(),
                    groups.getOrDefault(entry.getValue(), ""), result.size()));
            }
            return result;
        }

        /** 把本机改成 `target`，返回成功写入的次数和本机收不下的正文（过长、超出条数上限或写入失败；已经有的不算）。 */
        private PhraseApply applyPhrases(Map<String, String> local, List<SyncMergePolicy.Phrase> target) {
            SyncMergePolicy.LocalPlan plan = SyncMergePolicy.localPlan(local, target);
            int writes = 0;
            for (String id : plan.remove()) {
                CommonPhrasesStore.Result removed = CommonPhrasesStore.remove(context, id);
                if (removed.ok()) writes++;
                else Log.w(TAG, "phrase remove skipped: " + removed.failure());
            }
            String duplicate = CommonPhrasesStore.failureMessage("common_phrases_duplicate");
            Set<String> unheld = new HashSet<>(plan.add().size());
            for (String text : plan.add()) {
                if (!CommonPhrasesStore.validText(text)) {
                    unheld.add(text);
                    continue;
                }
                CommonPhrasesStore.Result added = CommonPhrasesStore.add(context, text);
                if (added.ok()) {
                    writes++;
                } else if (!duplicate.equals(added.failure())) {
                    unheld.add(text);
                    Log.w(TAG, "phrase add skipped: " + added.failure());
                }
            }
            if (!unheld.isEmpty()) Log.i(TAG, unheld.size() + " cloud phrases kept in the cloud but not on this device");
            return new PhraseApply(writes, unheld);
        }

        // ---- 个人词库 ----

        private void dictionary() throws CloudApi.Failure, IOException, JSONException, DictionarySnapshotQueue.Failure {
            if (!current()) return;
            boolean firstRun = first(SyncSwitch.DICTIONARY);
            boolean dirty = SyncSwitch.dirty(context, SyncSwitch.DICTIONARY);
            SyncMergePolicy.Mode mode;
            if (firstRun) {
                boolean localHasWords = dictionary.changed() && choice == SyncMergePolicy.Choice.MERGE && userWordCount() > 0;
                mode = SyncMergePolicy.dictionaryFirstRun(dictionary.changed(), localHasWords, choice);
                if (mode == null) mode = SyncMergePolicy.Mode.NONE;
            } else {
                mode = SyncMergePolicy.incremental(dictionary.changed(), dirty);
            }
            Path work = workDirectory();
            if (mode == SyncMergePolicy.Mode.UPLOAD) {
                if (SyncMergePolicy.uploadBlocked(pendingQueueCount())) {
                    Log.i(TAG, "dictionary upload postponed until the keyboard applies queued words");
                    return;
                }
                Path file = work.resolve("upload.ndjson");
                long generation = SyncSwitch.generation(context, SyncSwitch.DICTIONARY);
                try {
                    exportSnapshot(file);
                    try {
                        if (!current()) return;
                        long revision = api.uploadSnapshot(file, dictionary.revision());
                        SyncSwitch.setCursorIfCurrent(context, SyncSwitch.DICTIONARY, Long.toString(revision), bindingGeneration);
                        // 导出之后又有词库改动时代数已经变大，标记留着，下一轮再整份上传。
                        SyncSwitch.clearDirtyIfCurrent(context, SyncSwitch.DICTIONARY, generation, bindingGeneration);
                        return;
                    } catch (CloudApi.Failure failure) {
                        if (!SyncApi.conflict(failure)) throw failure;
                        // 云端在我们探测之后变了：并入云端的词，标记保留，等键盘应用后下一轮再整份上传。
                        mode = SyncMergePolicy.Mode.MERGE;
                    }
                } finally {
                    Files.deleteIfExists(file);
                }
            }
            if (mode == SyncMergePolicy.Mode.NONE) {
                if (firstRun) SyncSwitch.setCursorIfCurrent(context, SyncSwitch.DICTIONARY,
                    Long.toString(dictionary.revision()), bindingGeneration);
                return;
            }
            Path file = work.resolve("download.ndjson");
            try {
                long revision = api.downloadSnapshot(file);
                if (mode == SyncMergePolicy.Mode.DOWNLOAD) {
                    synchronized (SyncSwitch.bindingLock()) {
                        if (!current()) return;
                        enqueueSnapshot(file, revision, accountId);
                    }
                    // The queue checks expectedLocalVersion again when it activates. A
                    // dictionary edit can therefore happen after this download and make the
                    // request conflict instead of overwriting that edit. Keep the dirty mark so
                    // the next round uploads the local version after that conflict; clearing it
                    // here would lose the only signal that the local edit needs syncing.
                } else {
                    List<SyncMergePolicy.Word> words = SyncApi.snapshotWords(file);
                    synchronized (SyncSwitch.bindingLock()) {
                        if (!current()) return;
                        importWords(words);
                        SyncSwitch.markDirtyIfCurrent(context, SyncSwitch.DICTIONARY, bindingGeneration);
                    }
                }
                if (!current()) return;
                SyncSwitch.setCursorIfCurrent(context, SyncSwitch.DICTIONARY, Long.toString(revision), bindingGeneration);
            } finally {
                Files.deleteIfExists(file);
            }
        }

        private Path workDirectory() throws IOException {
            File files = context.getFilesDir();
            if (files == null) throw new IOException("private files unavailable");
            Path work = files.toPath().resolve(WORK_PATH);
            SafePaths.ensureDirectory(work);
            return work;
        }

        private String hostOptions() throws IOException {
            return CloudSync.hostOptions(context);
        }

        private int userWordCount() throws IOException, JSONException {
            return CloudSync.userWordCount(context);
        }

        private int pendingQueueCount() throws IOException, JSONException {
            JSONObject value = nativeValue(NativeClient.personalDictionaryRequest(new JSONObject()
                .put("options", new JSONObject(hostOptions()))
                .put("action", new JSONObject().put("operation", "list").put("offset", 0).put("limit", 1)
                    .put("user_only", true)).toString()));
            Integer pending = DictionaryCollectionsStore.nonNegativeInteger(value.opt("pending_count"));
            return pending == null ? 0 : pending;
        }

        /** 云同步上传的快照只有词，不带输入记录（学习调权、固定位置和选词计数只进本地备份）。 */
        private void exportSnapshot(Path destination) throws IOException, JSONException {
            exportDictionarySnapshot(context, destination, false);
        }

        /** 下载的快照交给现有的激活队列，键盘下次没有会话时整份激活。 */
        private void enqueueSnapshot(Path file, long revision, String accountId)
                throws IOException, DictionarySnapshotQueue.Failure {
            enqueueDictionarySnapshot(context, file, accountId, revision);
        }

        /** 「合并」：把云端的词经个人词库队列导入本机，键盘下次开会话时应用。 */
        private void importWords(List<SyncMergePolicy.Word> words) throws IOException, JSONException {
            if (!current()) return;
            int skipped = queueWords(context, words, "cloud-merge-");
            if (skipped > 0) Log.w(TAG, skipped + " cloud words skipped during merge");
        }
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
        KeyboardFeedbackStore.Settings settings = KeyboardFeedbackStore.load(context);
        return new JSONObject()
            .put("soundEnabled", settings.soundEnabled())
            .put("hapticsEnabled", settings.hapticsEnabled())
            .put("hapticStrength", settings.hapticStrength().id());
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
        Integer count = DictionaryCollectionsStore.nonNegativeInteger(value.opt("count"));
        return count == null ? 0 : count;
    }

    /** 把一份词库快照交给激活队列，键盘下次没有会话时用它整份替换用户词库。键盘还没发布过本机词库版本时抛 IOException。`owner` 是请求的来源：云同步是账号 id，本地备份是固定的标记。 */
    static void enqueueDictionarySnapshot(Context context, Path file, String owner, long revision)
            throws IOException, DictionarySnapshotQueue.Failure {
        File files = context.getFilesDir();
        if (files == null) throw new IOException("private files unavailable");
        Path root = files.toPath().toAbsolutePath().normalize();
        DictionarySnapshotQueue queue = new DictionarySnapshotQueue(root, root.resolve(QUEUE_PATH));
        String localVersion = queue.read().localVersion();
        if (localVersion == null) throw new IOException("keyboard has not published a dictionary version yet");
        queue.enqueue(file.toAbsolutePath(), owner, revision, localVersion, DigestPolicy.sha256Hex(file));
    }

    /**
     * 经个人词库队列导入一批词，键盘下次开会话时应用；已有的词由队列合并，不会重复。整批被拒时逐条再试，坏的那条跳过。云同步的「合并」和本地备份的恢复都用它。
     *
     * @param requestPrefix 队列请求 id 的前缀，区分来源（只进日志）
     * @return 没能排进队列的词数
     */
    static int queueWords(Context context, List<SyncMergePolicy.Word> words, String requestPrefix)
            throws IOException, JSONException {
        String options = hostOptions(context);
        int skipped = 0;
        for (List<SyncMergePolicy.Word> batch : SyncMergePolicy.batches(words, SyncMergePolicy.PERSONAL_IMPORT_BATCH)) {
            if (queueImport(options, batch, requestPrefix)) continue;
            for (SyncMergePolicy.Word word : batch) {
                if (!queueImport(options, List.of(word), requestPrefix)) skipped++;
            }
        }
        return skipped;
    }

    private static boolean queueImport(String options, List<SyncMergePolicy.Word> words, String requestPrefix)
            throws JSONException {
        JSONArray entries = new JSONArray();
        for (SyncMergePolicy.Word word : words) {
            entries.put(new JSONObject().put("kind", word.kind()).put("key", word.key())
                .put("value", word.value()).put("weight", word.weight()));
        }
        String file = new JSONObject().put("format", "msime-personal-dictionary").put("version", 1)
            .put("entries", entries).toString();
        JSONObject response = new JSONObject(NativeClient.personalDictionaryRequest(new JSONObject()
            .put("options", new JSONObject(options))
            .put("action", new JSONObject().put("operation", "import_personal").put("text", file)
                .put("request_id", requestPrefix + UUID.randomUUID())).toString()));
        return JsonPolicy.strictTrue(response.opt("ok"));
    }

    // ---- JSON 小工具 ----

    /** client-core 的标准响应 `{ok, value, error}`：失败时抛出，信息只进日志。 */
    static JSONObject nativeValue(String response) throws JSONException {
        JSONObject root = new JSONObject(response == null ? "" : response);
        if (!JsonPolicy.strictTrue(root.opt("ok")))
            throw new IllegalStateException(root.optString("error", "native call failed"));
        JSONObject value = root.optJSONObject("value");
        return value == null ? new JSONObject() : value;
    }

    private static JSONObject document(long revision, Map<String, Object> settings) throws JSONException {
        JSONObject values = new JSONObject();
        for (Map.Entry<String, Object> entry : settings.entrySet()) values.put(entry.getKey(), entry.getValue());
        return new JSONObject().put("revision", revision).put("settings", values);
    }

    private static JSONObject settingsOf(JSONObject document) throws JSONException {
        return document.getJSONObject("settings");
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
