package app.msime.android.home;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.util.Log;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppEdition;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.LocalBackupPolicy;
import app.msime.android.NativeClient;
import app.msime.android.R;
import app.msime.android.SafePaths;
import app.msime.android.SyncApi;
import app.msime.android.SyncMergePolicy;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.TextPolicy;
import java.io.BufferedOutputStream;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.time.Instant;
import java.time.LocalDateTime;
import java.time.temporal.ChronoUnit;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 本地备份与恢复（#5659）：把设置、Android 本地设置、自定义皮肤、常用语和个人词库打成一个 zip，存到用户在系统文件选择器里选的位置；换手机或降级时再从这个文件恢复。全程只在本机，不经过云端，也不需要登录。包的格式见 {@link LocalBackupPolicy}。
 *
 * <p>内容与云同步相同，复用同一套导出和应用：设置文档走 client-core 的 `msime_client_account_settings_export`/`_apply`（凭据与诊断日志永远不在里面），个人词库走 `export_snapshot`（{@link CloudSync#exportDictionarySnapshot}）。恢复是合并而不是覆盖：设置按备份里的值改写，皮肤按 id 和更新时间合并，常用语只加本机还没有的，词经命名词库的待发送队列在键盘空闲时陆续写入，本机已有的词不会被删掉。
 *
 * <p>所有方法都会读写文件、调用原生库，只能在工作线程上调。
 */
final class LocalBackup {
    private static final String TAG = "MSIMEBackup";
    /** 应用缓存下的工作目录：导出时先在这里拼好整个包再写到用户选的位置，恢复时先把选中的文件复制到这里。 */
    private static final String WORK = "backup";
    private static final int COPY_BUFFER = 64 * 1024;
    /** 本地恢复交给词库快照激活队列时的请求来源；云同步的请求用账号 id，退出登录时只取消自己的。 */
    private static final String SNAPSHOT_OWNER = "local-backup";

    private LocalBackup() {}

    /** 系统文件选择器里预填的文件名：应用名、版本号和导出时间。 */
    static String defaultName(Context context) {
        return LocalBackupPolicy.fileName(context.getString(R.string.app_name),
            UpdateJobService.currentVersion(context), LocalDateTime.now());
    }

    // ---- 导出 ----

    /** 导出到 `uri`，返回给用户看的结果。失败时尽量删掉选择器已经建好的空文件。 */
    static String export(Context context, Uri uri) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return "还没有完成首次准备，暂时没有可以备份的数据。";
        Path dictionary = null;
        Path archive = null;
        boolean written = false;
        try {
            Path work = workDirectory(context);
            dictionary = work.resolve("export-dictionary.ndjson");
            archive = work.resolve("export.zip");
            JSONObject settings = CloudSync.nativeValue(NativeClient.accountSettingsExport(
                new JSONObject().put("preferences_directory", directory)
                    .put("feedback", CloudSync.hostFeedback(context)).toString()))
                .getJSONObject("settings");
            settings.remove(SyncMergePolicy.SKINS_KEY);
            JSONObject local = new JSONObject();
            for (Map.Entry<String, Object> entry : AndroidLocalSettings.load(context).explicit().entrySet()) {
                if (LocalBackupPolicy.backsUpLocalSetting(entry.getKey())) local.put(entry.getKey(), entry.getValue());
            }
            String skins = CustomSkinLibrary.exportDesigns(Paths.get(directory));
            List<String> phrases = ownPhrases(context);
            JSONObject snapshot = CloudSync.exportDictionarySnapshot(context, dictionary);
            Integer words = DictionaryCollectionsStore.nonNegativeInteger(snapshot.opt("entries"));
            int skinCount = new JSONArray(skins).length();

            JSONObject manifest = new JSONObject()
                .put("format", LocalBackupPolicy.FORMAT)
                .put("version", LocalBackupPolicy.VERSION)
                .put("app", context.getString(R.string.app_name))
                .put("app_version", UpdateJobService.currentVersion(context))
                .put("edition", AppEdition.current().id())
                .put("created_at", Instant.now().truncatedTo(ChronoUnit.SECONDS).toString())
                .put("contents", new JSONObject()
                    .put("settings", settings.length())
                    .put("android_local", local.length())
                    .put("skins", skinCount)
                    .put("phrases", phrases.size())
                    .put("dictionary_words", words == null ? 0 : words));
            try (ZipOutputStream zip = new ZipOutputStream(new BufferedOutputStream(Files.newOutputStream(archive)))) {
                putText(zip, LocalBackupPolicy.MANIFEST, manifest.toString());
                putText(zip, LocalBackupPolicy.SETTINGS, new JSONObject().put("settings", settings).toString());
                putText(zip, LocalBackupPolicy.ANDROID_LOCAL, local.toString());
                putText(zip, LocalBackupPolicy.SKINS, skins);
                putText(zip, LocalBackupPolicy.PHRASES, new JSONObject().put("phrases", new JSONArray(phrases)).toString());
                zip.putNextEntry(new ZipEntry(LocalBackupPolicy.DICTIONARY));
                Files.copy(dictionary, zip);
                zip.closeEntry();
            }
            ContentResolver resolver = context.getContentResolver();
            try (OutputStream output = resolver.openOutputStream(uri, "wt")) {
                if (output == null) throw new FileNotFoundException("backup destination unavailable");
                written = true;
                Files.copy(archive, output);
            }
            return "已导出备份：" + phrases.size() + " 条常用语、" + (words == null ? 0 : words) + " 个词，以及全部设置。";
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "backup export failed", error);
            discardDocument(context, uri, written);
            return "没有导出成功，请稍后重试。键盘正在整理词库时会暂时导不出来。";
        } finally {
            deleteQuietly(dictionary);
            deleteQuietly(archive);
        }
    }

    /** 自己添加的常用语，按本机顺序；本机预置、没被用户认领过的示例不算，它们在每台设备上都会自己出现。 */
    private static List<String> ownPhrases(Context context) throws IOException {
        CommonPhrasesStore.Result result = CommonPhrasesStore.load(context);
        if (!result.ok()) throw new IOException("common phrases unavailable");
        Set<String> starters = CommonPhrasesStore.untouchedStarters(context);
        List<String> texts = new ArrayList<>(result.document().phrases().size());
        for (CommonPhrasesStore.Phrase phrase : result.document().phrases()) {
            if (phrase.own() && !starters.contains(phrase.text())) texts.add(phrase.text());
        }
        return texts;
    }

    private static void putText(ZipOutputStream zip, String name, String text) throws IOException {
        zip.putNextEntry(new ZipEntry(name));
        zip.write(TextPolicy.utf8Bytes(text));
        zip.closeEntry();
    }

    /**
     * 导出失败时删掉选择器建好的空文件或写了一半的包，不在用户的目录里留一个打不开的包；删不掉（提供方不支持）就算了。
     *
     * <p>还没开始写（`written` 为假）时只删空文件：用户在选择器里可以点一个已有的文件确认覆盖，这时拿到的是那个文件本身，在打包阶段失败（例如键盘正在整理词库）就删掉它，等于把用户原来的备份删了。大小读不出来时同样不删，最坏是留下一个空文件。
     */
    private static void discardDocument(Context context, Uri uri, boolean written) {
        if (!written && !emptyDocument(context, uri)) return;
        try {
            DocumentsContract.deleteDocument(context.getContentResolver(), uri);
        } catch (FileNotFoundException | RuntimeException ignored) {
            // 有的文档提供方不支持删除；留下的是空文件，不影响别的数据。
        }
    }

    /** 选择器给的文档确定是 0 字节时返回 true；读不出大小时返回 false。 */
    private static boolean emptyDocument(Context context, Uri uri) {
        try (Cursor cursor = context.getContentResolver().query(uri, new String[] {OpenableColumns.SIZE}, null, null, null)) {
            return cursor != null && cursor.moveToFirst() && !cursor.isNull(0) && cursor.getLong(0) == 0;
        } catch (RuntimeException unreadable) {
            return false;
        }
    }

    // ---- 恢复 ----

    /** 选中的备份包已经复制进工作目录，等用户确认后恢复；`preview` 是给确认对话框看的说明。 */
    record Prepared(Path archive, Preview preview) {}

    /** 确认对话框里显示的备份说明。 */
    record Preview(LocalBackupPolicy.Compatibility compatibility, String app, String appVersion, String createdAt,
            int phrases, int words) {}

    /** 把用户选的文件复制进工作目录并读出说明；读不出来时返回 null，并删掉复制的那份。确认后交给 {@link #restore}，取消时交给 {@link #discard}。 */
    @Nullable static Prepared prepare(Context context, Uri uri) {
        Path archive = null;
        try {
            archive = copyIn(context, uri, "restore.zip");
            try (ZipFile zip = new ZipFile(archive.toFile())) {
                JSONObject manifest = jsonEntry(zip, LocalBackupPolicy.MANIFEST, LocalBackupPolicy.MAX_MANIFEST_BYTES);
                LocalBackupPolicy.Compatibility compatibility = manifest == null
                    ? LocalBackupPolicy.Compatibility.NOT_A_BACKUP
                    : LocalBackupPolicy.compatibility(manifest.opt("format"), manifest.opt("version"));
                JSONObject contents = manifest == null ? null : manifest.optJSONObject("contents");
                Preview preview = new Preview(compatibility,
                    manifest == null ? "" : manifest.optString("app", ""),
                    manifest == null ? "" : manifest.optString("app_version", ""),
                    manifest == null ? "" : manifest.optString("created_at", ""),
                    contents == null ? 0 : Math.max(0, contents.optInt("phrases", 0)),
                    contents == null ? 0 : Math.max(0, contents.optInt("dictionary_words", 0)));
                if (compatibility != LocalBackupPolicy.Compatibility.OK) discard(archive);
                return new Prepared(archive, preview);
            }
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "backup preview failed", error);
            discard(archive);
            return null;
        }
    }

    static void discard(@Nullable Path archive) {
        deleteQuietly(archive);
    }

    /** 从 {@link #prepare} 复制好的备份包恢复，返回给用户看的结果；恢复完删掉那份副本。各部分互不牵连：一部分失败照样恢复其余部分，并在结果里说出没恢复的部分。 */
    static String restore(Context context, Path archive) {
        String directory = HostStore.directory(context);
        Path dictionary = archive.resolveSibling("restore-dictionary.ndjson");
        try {
            if (directory.isEmpty()) return "请先打开一次键盘完成首次准备，再来恢复。";
            try (ZipFile zip = new ZipFile(archive.toFile())) {
                List<String> failed = new ArrayList<>(4);
                boolean settings = restoreSettings(context, directory, zip, failed);
                int skins = restoreSkins(context, directory, zip, failed);
                int phrases = restorePhrases(context, zip, failed);
                int[] words = restoreDictionary(context, zip, dictionary, failed);
                return LocalBackupPolicy.summary(new LocalBackupPolicy.Restored(settings, skins, phrases, words[0],
                    words[1], failed));
            }
        } catch (IOException | RuntimeException error) {
            Log.w(TAG, "backup restore failed", error);
            return "读不出这份备份文件，请确认选的是导出的 .zip 文件。";
        } finally {
            deleteQuietly(dictionary);
            deleteQuietly(archive);
        }
    }

    /** 设置文档按备份里的值声明字段类型后交给 client-core 应用，再写回 Android 本地设置。成功应用了设置文档时返回 true。 */
    private static boolean restoreSettings(Context context, String directory, ZipFile zip, List<String> failed) {
        boolean applied = false;
        try {
            JSONObject document = jsonEntry(zip, LocalBackupPolicy.SETTINGS, LocalBackupPolicy.MAX_SETTINGS_BYTES);
            JSONObject settings = document == null ? null : document.optJSONObject("settings");
            if (settings != null && settings.length() > 0) {
                settings.remove(SyncMergePolicy.SKINS_KEY);
                Map<String, String> types = LocalBackupPolicy.fieldTypes(CloudSync.map(settings));
                if (types == null) throw new IOException("too many settings");
                JSONObject fields = new JSONObject();
                for (Map.Entry<String, String> type : types.entrySet()) {
                    fields.put(type.getKey(), new JSONObject().put("type", type.getValue()));
                }
                JSONObject schema = new JSONObject().put("fields", fields).put("maximum_bytes", 1024 * 1024)
                    .put("update_mode", "replace").put("revision_required", true);
                JSONObject cloud = new JSONObject().put("revision", 0).put("settings", settings);
                try {
                    CloudSync.applySettings(context, directory, cloud, schema);
                } catch (IllegalStateException changed) {
                    // 偏好按读到的修订号比较并交换；键盘恰好在这时写了一次偏好就重读重来一次。
                    CloudSync.applySettings(context, directory, cloud, schema);
                }
                SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                applied = true;
            }
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "settings restore failed", error);
            failed.add("设置");
        }
        try {
            JSONObject local = jsonEntry(zip, LocalBackupPolicy.ANDROID_LOCAL, LocalBackupPolicy.MAX_ANDROID_LOCAL_BYTES);
            if (local != null) {
                Map<String, Object> edits = new LinkedHashMap<>(local.length());
                Map<String, AndroidLocalSettings.Spec> specs = AndroidLocalSettings.specs();
                for (Iterator<String> keys = local.keys(); keys.hasNext(); ) {
                    String key = keys.next();
                    AndroidLocalSettings.Spec spec = specs.get(key);
                    // 别的版本写下、本版本不认识或取值不合规的键跳过，不让整份本地设置因为一项失败；语音数据贡献和开发者选项即使在包里也不恢复。
                    Object value = spec == null || !LocalBackupPolicy.backsUpLocalSetting(key) ? null : spec.accept(local.opt(key));
                    if (value != null) edits.put(key, value);
                }
                if (!edits.isEmpty()) {
                    AndroidLocalSettings.update(context, edits);
                    // 本地设置里有一部分随设置同步（应用主题、按键细节等），写回后同样标记待上传。
                    SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                    applied = true;
                }
            }
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "android local settings restore failed", error);
            if (!failed.contains("设置")) failed.add("键盘设置");
        }
        return applied;
    }

    private static int restoreSkins(Context context, String directory, ZipFile zip, List<String> failed) {
        try {
            byte[] bytes = entryBytes(zip, LocalBackupPolicy.SKINS, LocalBackupPolicy.MAX_SKINS_BYTES);
            if (bytes == null) return 0;
            int changed = CustomSkinLibrary.importDesigns(Paths.get(directory), TextPolicy.utf8(bytes));
            if (changed > 0) SyncSignals.markDirty(context, SyncSwitch.SKINS);
            return changed;
        } catch (IOException | RuntimeException error) {
            Log.w(TAG, "skin restore failed", error);
            failed.add("自定义皮肤");
            return 0;
        }
    }

    /** 逐条加回常用语；本机已经有的跳过。返回新加的条数。本机已有的同一段正文若是本机预置、还没被认领的示例（新手机打开过常用语就会放进去），把它认领成用户的常用语：在旧手机上它是用户自己的，不认领的话云同步会一直跳过它。 */
    private static int restorePhrases(Context context, ZipFile zip, List<String> failed) {
        try {
            JSONObject document = jsonEntry(zip, LocalBackupPolicy.PHRASES, LocalBackupPolicy.MAX_PHRASES_BYTES);
            JSONArray phrases = document == null ? null : document.optJSONArray("phrases");
            if (phrases == null) return 0;
            String duplicate = CommonPhrasesStore.failureMessage("common_phrases_duplicate");
            int added = 0;
            int unheld = 0;
            List<String> present = new ArrayList<>();
            for (int index = 0; index < phrases.length(); index++) {
                if (!(phrases.opt(index) instanceof String text) || !CommonPhrasesStore.validText(text)) {
                    unheld++;
                    continue;
                }
                CommonPhrasesStore.Result result = CommonPhrasesStore.add(context, text);
                if (result.ok()) added++;
                else if (duplicate.equals(result.failure())) present.add(text);
                else unheld++;
            }
            if (!present.isEmpty()) {
                try {
                    CommonPhrasesStore.adoptStarters(context, present);
                } catch (IOException error) {
                    // 正文都已经在本机了，认领失败最坏是这几条暂时不参与同步，和 CommonPhrasesStore.add 的处理相同。
                    Log.w(TAG, "starter record was not updated", error);
                }
            }
            if (unheld > 0) failed.add(unheld + " 条常用语");
            return added;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "phrase restore failed", error);
            failed.add("常用语");
            return 0;
        }
    }

    /**
     * 个人词库：解出快照。本机用户词库还是空的（新手机、重装）时把整份快照交给激活队列，原样恢复，与云同步第一次同步而本机没有词时的做法相同。本机已经有词时（或激活队列用不了时）合并，已有的词不删：词先全部记进命名词库的待发送队列（{@link DictionaryCollectionsStore#queueUnownedWords}），再在键盘空闲时一批批送进个人词库队列。个人词库队列同时只收 128 个未完成的请求，所以不能直接把几千个词塞进去（那样第 129 个以后的词全会被拒）。返回 {恢复的词数, 跳过的词数}。
     */
    private static int[] restoreDictionary(Context context, ZipFile zip, Path file, List<String> failed) {
        try {
            ZipEntry entry = zip.getEntry(LocalBackupPolicy.DICTIONARY);
            if (entry == null) return new int[] {0, 0};
            try (InputStream input = zip.getInputStream(entry)) {
                copyBounded(input, file, LocalBackupPolicy.MAX_DICTIONARY_BYTES);
            }
            List<SyncMergePolicy.Word> words = SyncApi.snapshotWords(file);
            if (words.isEmpty()) return new int[] {0, 0};
            if (CloudSync.userWordCount(context) == 0) {
                try {
                    CloudSync.enqueueDictionarySnapshot(context, file, SNAPSHOT_OWNER, 0);
                    SyncSignals.markDirty(context, SyncSwitch.DICTIONARY);
                    return new int[] {words.size(), 0};
                } catch (IOException | DictionarySnapshotQueue.Failure unavailable) {
                    // 键盘还没打开过（没有发布本机词库版本）或者已有一份快照在排队：退回导入队列。
                    Log.i(TAG, "snapshot activation unavailable, merging instead", unavailable);
                }
            }
            int queued = 0;
            String failure = null;
            for (List<SyncMergePolicy.Word> batch : SyncMergePolicy.batches(words, DictionaryCollectionsStore.MAX_QUEUED_WORDS)) {
                DictionaryCollectionsStore.Result<Integer> result = DictionaryCollectionsStore.queueUnownedWords(context, batch);
                if (!result.ok()) {
                    failure = result.failure();
                    break;
                }
                queued += result.value();
            }
            if (queued > 0) SyncSignals.markDirty(context, SyncSwitch.DICTIONARY);
            if (failure != null) {
                // 已经排进去的词照样写入；没排进去的那部分如实报告，再恢复一次时已排的词不会重复。
                Log.w(TAG, "dictionary restore stopped: " + failure);
                failed.add(queued > 0 ? "其余的个人词库" : "个人词库");
                return new int[] {queued, 0};
            }
            return new int[] {queued, words.size() - queued};
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "dictionary restore failed", error);
            failed.add("个人词库");
            return new int[] {0, 0};
        }
    }

    // ---- 文件 ----

    private static Path workDirectory(Context context) throws IOException {
        File cache = context.getCacheDir();
        if (cache == null) throw new IOException("cache directory unavailable");
        Path work = cache.toPath().resolve(WORK);
        SafePaths.ensureDirectory(work);
        return work;
    }

    /** 把用户选的文件复制进工作目录，超过 {@link LocalBackupPolicy#MAX_ARCHIVE_BYTES} 时放弃。 */
    private static Path copyIn(Context context, Uri uri, String name) throws IOException {
        Path target = workDirectory(context).resolve(name);
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) throw new FileNotFoundException("backup source unavailable");
            copyBounded(input, target, LocalBackupPolicy.MAX_ARCHIVE_BYTES);
        }
        return target;
    }

    /** 先写到同目录的临时文件，超过 `limit` 字节就删掉并失败，写完再改名。 */
    private static void copyBounded(InputStream input, Path target, long limit) throws IOException {
        Path temporary = Files.createTempFile(target.getParent(), target.getFileName().toString(), ".part");
        try {
            try (OutputStream output = Files.newOutputStream(temporary)) {
                byte[] buffer = new byte[COPY_BUFFER];
                long total = 0;
                int count;
                while ((count = input.read(buffer)) != -1) {
                    total += count;
                    if (total > limit) throw new IOException("backup entry too large");
                    output.write(buffer, 0, count);
                }
            }
            Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    @Nullable private static byte[] entryBytes(ZipFile zip, String name, int limit) throws IOException {
        ZipEntry entry = zip.getEntry(name);
        if (entry == null) return null;
        try (InputStream input = zip.getInputStream(entry)) {
            return HttpBodyPolicy.readRequired(input, limit);
        }
    }

    @Nullable private static JSONObject jsonEntry(ZipFile zip, String name, int limit) throws IOException, JSONException {
        byte[] bytes = entryBytes(zip, name, limit);
        return bytes == null ? null : new JSONObject(TextPolicy.utf8(bytes));
    }

    private static void deleteQuietly(@Nullable Path path) {
        if (path == null) return;
        try {
            Files.deleteIfExists(path);
        } catch (IOException ignored) {
            // 缓存目录里的临时文件，系统清缓存时也会删掉。
        }
    }
}
