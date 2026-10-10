package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import android.util.Log;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppEdition;
import app.msime.android.BackupDestinationWriter;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.DigestPolicy;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.KeyboardFeedbackStore;
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
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
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
import java.util.Objects;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipOutputStream;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 本地备份与恢复（#5659）：把设置、Android 本地设置、自定义皮肤、常用语、个人词库和输入记录打成一个 zip，存到用户在系统文件选择器里选的位置；换手机或降级时再从这个文件恢复。全程只在本机，不经过云端，也不需要登录。包的格式见 {@link LocalBackupPolicy}。
 *
 * <p>内容大体与云同步相同，复用同一套导出和应用：设置文档走 client-core 的 `msime_client_account_settings_export`/`_apply`（凭据与诊断日志永远不在里面），个人词库走 `export_snapshot`（{@link CloudSync#exportDictionarySnapshot}）。只有输入记录是云同步没有的：学习调权、删除记录、固定位置和选词计数由 `export_snapshot` 一并写进词库快照，快照格式装不下的输入习惯（整句联想、连续选词、拼写纠错、自动纠错抑制、置顶）由 `export_habits` 另写一个条目。恢复是合并而不是覆盖：设置按备份里的值改写，皮肤按 id 和更新时间合并，常用语只加本机还没有的，词经命名词库的待发送队列在键盘空闲时陆续写入，本机已有的词不会被删掉；输入记录和输入习惯排给键盘在收起后空闲时合并，本机已有的保留本机，计数取较大的那个。
 *
 * <p>恢复分两步，先校验再动本机：{@link #prepare} 核对 manifest 记的校验和，再逐项解析设置、皮肤和常用语，并让原生侧完整校验词库快照和输入习惯，任何一项不过都不进入恢复（{@link LocalBackupPolicy.Compatibility#DAMAGED}）。{@link #restore} 先改设置、皮肤和常用语，改之前记下本机原来的样子，其中任何一部分写入失败就全部撤销回去，词库和输入记录也不再动；这几部分都成功后才排词库和输入记录，它们是只增不删、本机优先的合并，排到一半失败时已排的照样写入，再恢复一次不会重复。
 *
 * <p>所有方法都会读写文件、调用原生库，只能在工作线程上调。
 */
final class LocalBackup {
    private static final String TAG = "MSIMEBackup";
    /** 应用缓存下的工作目录：导出时先在这里拼好整个包再写到用户选的位置，恢复时先把选中的文件复制到这里。 */
    private static final String WORK = "backup";
    private static final int COPY_BUFFER = 64 * 1024;
    /** 导出页和导出结果里都要说的话：输入记录能看出打字习惯，备份文件没有加密。 */
    static final String PRIVACY_NOTICE = "备份文件里有你的输入记录，能看出你常打的字词和打字习惯。文件没有加密，请妥善保管，不要发给别人或传到公开的地方。";

    private LocalBackup() {}

    /** 系统文件选择器里预填的文件名：应用名、版本号和导出时间。 */
    static String defaultName(Context context) {
        return LocalBackupPolicy.fileName(context.getString(R.string.app_name),
            UpdateJobService.currentVersion(context), LocalDateTime.now());
    }

    // ---- 导出 ----

    /** 导出到 `uri`，返回给用户看的结果。失败时保留提供方返回的文档。 */
    static String export(Context context, Uri uri) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return "还没有完成首次准备，暂时没有可以备份的数据。";
        Path dictionary = null;
        Path habitsFile = null;
        Path archive = null;
        try {
            Path work = workDirectory(context);
            dictionary = work.resolve("export-dictionary.ndjson");
            habitsFile = work.resolve("export-habits.ndjson");
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
            JSONObject snapshot = CloudSync.exportDictionarySnapshot(context, dictionary, true);
            Integer words = DictionaryCollectionsStore.nonNegativeInteger(snapshot.opt("entries"));
            Integer learning = DictionaryCollectionsStore.nonNegativeInteger(snapshot.opt("learning"));
            // 输入记录读不出来时原生侧照样导出词，只是不带输入记录（`learning_error`）。
            boolean learningMissing = snapshot.optString("learning_error", "").length() > 0;
            if (learningMissing) Log.w(TAG, "backup exported without input records: " + snapshot.optString("learning_error"));
            Integer habits = exportHabits(context, habitsFile);
            // 输入习惯读不出来时同样不让整份备份失败，只是包里没有这一项。
            if (habits == null) learningMissing = true;
            int skinCount = new JSONArray(skins).length();
            Map<String, byte[]> texts = new LinkedHashMap<>();
            texts.put(LocalBackupPolicy.SETTINGS, TextPolicy.utf8Bytes(new JSONObject().put("settings", settings).toString()));
            texts.put(LocalBackupPolicy.ANDROID_LOCAL, TextPolicy.utf8Bytes(local.toString()));
            texts.put(LocalBackupPolicy.SKINS, TextPolicy.utf8Bytes(skins));
            texts.put(LocalBackupPolicy.PHRASES,
                TextPolicy.utf8Bytes(new JSONObject().put("phrases", new JSONArray(phrases)).toString()));
            JSONObject checksums = new JSONObject();
            for (Map.Entry<String, byte[]> text : texts.entrySet()) checksums.put(text.getKey(), sha256(text.getValue()));
            checksums.put(LocalBackupPolicy.DICTIONARY, DigestPolicy.sha256Hex(dictionary));
            if (habits != null) checksums.put(LocalBackupPolicy.HABITS, DigestPolicy.sha256Hex(habitsFile));

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
                    .put("dictionary_words", words == null ? 0 : words)
                    .put("learning", learning == null ? 0 : learning)
                    .put("habits", habits == null ? 0 : habits))
                .put(LocalBackupPolicy.CHECKSUMS, checksums);
            try (ZipOutputStream zip = new ZipOutputStream(new BufferedOutputStream(Files.newOutputStream(archive)))) {
                putBytes(zip, LocalBackupPolicy.MANIFEST, TextPolicy.utf8Bytes(manifest.toString()));
                for (Map.Entry<String, byte[]> text : texts.entrySet()) putBytes(zip, text.getKey(), text.getValue());
                putFile(zip, LocalBackupPolicy.DICTIONARY, dictionary);
                if (habits != null) putFile(zip, LocalBackupPolicy.HABITS, habitsFile);
            }
            BackupDestinationWriter.write(archive, new BackupDestinationWriter.Destination() {
                @Override public InputStream openForRead() throws FileNotFoundException {
                    return context.getContentResolver().openInputStream(uri);
                }

                @Override public OutputStream openForWrite() throws FileNotFoundException {
                    return context.getContentResolver().openOutputStream(uri, "wt");
                }
            });
            int records = (learning == null ? 0 : learning) + (habits == null ? 0 : habits);
            if (learningMissing) {
                return "已导出备份：" + phrases.size() + " 条常用语、" + (words == null ? 0 : words) + " 个词、"
                    + records + " 条输入记录，以及全部设置。"
                    + "有一部分输入记录没能读出来，没有放进这份备份。\n\n" + PRIVACY_NOTICE;
            }
            return "已导出备份：" + phrases.size() + " 条常用语、" + (words == null ? 0 : words) + " 个词、"
                + records + " 条输入记录，以及全部设置。\n\n" + PRIVACY_NOTICE;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "backup export failed", error);
            return "没有导出成功，请稍后重试。若目标位置留下未完成的备份，请手动删除。";
        } finally {
            deleteQuietly(dictionary);
            deleteQuietly(habitsFile);
            deleteQuietly(archive);
        }
    }

    /** 把输入习惯写到 `file`（`export_habits`），返回条数；读不出来时返回 null 并删掉半成品，备份照样导出其余部分。 */
    @Nullable private static Integer exportHabits(Context context, Path file) {
        try {
            Files.deleteIfExists(file);
            JSONObject value = CloudSync.nativeValue(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", "export_habits")
                    .put("destination", file.toAbsolutePath().toString())).toString()));
            Integer habits = DictionaryCollectionsStore.nonNegativeInteger(value.opt("habits"));
            if (habits == null) throw new IOException("habits count missing");
            return habits;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "backup exported without input habits", error);
            deleteQuietly(file);
            return null;
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

    private static void putBytes(ZipOutputStream zip, String name, byte[] bytes) throws IOException {
        zip.putNextEntry(new ZipEntry(name));
        zip.write(bytes);
        zip.closeEntry();
    }

    private static void putFile(ZipOutputStream zip, String name, Path file) throws IOException {
        zip.putNextEntry(new ZipEntry(name));
        Files.copy(file, zip);
        zip.closeEntry();
    }

    private static MessageDigest sha256Digest() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static String sha256(byte[] bytes) {
        return DigestPolicy.hex(sha256Digest().digest(bytes));
    }

    // ---- 恢复 ----

    /** 选中的备份包已经复制进工作目录，等用户确认后恢复；`preview` 是给确认对话框看的说明。 */
    record Prepared(Path archive, Preview preview) {}

    /** 确认对话框里显示的备份说明。`learning` 含输入习惯的条数。 */
    record Preview(LocalBackupPolicy.Compatibility compatibility, String app, String appVersion, String createdAt,
            int phrases, int words, int learning) {}

    /** 把用户选的文件复制进工作目录，读出说明并整份校验（{@link #verify}）；读不出来时返回 null，并删掉复制的那份。确认后交给 {@link #restore}，取消时交给 {@link #discard}。 */
    @Nullable static Prepared prepare(Context context, Uri uri) {
        Path archive = null;
        try {
            archive = copyIn(context, uri, "restore.zip");
            try (ZipFile zip = new ZipFile(archive.toFile())) {
                JSONObject manifest = jsonEntry(zip, LocalBackupPolicy.MANIFEST, LocalBackupPolicy.MAX_MANIFEST_BYTES);
                LocalBackupPolicy.Compatibility compatibility = manifest == null
                    ? LocalBackupPolicy.Compatibility.NOT_A_BACKUP
                    : LocalBackupPolicy.compatibility(manifest.opt("format"), manifest.opt("version"));
                if (compatibility == LocalBackupPolicy.Compatibility.OK && !verify(context, zip, manifest, archive)) {
                    compatibility = LocalBackupPolicy.Compatibility.DAMAGED;
                }
                JSONObject contents = manifest == null ? null : manifest.optJSONObject("contents");
                Preview preview = new Preview(compatibility,
                    manifest == null ? "" : manifest.optString("app", ""),
                    manifest == null ? "" : manifest.optString("app_version", ""),
                    manifest == null ? "" : manifest.optString("created_at", ""),
                    contents == null ? 0 : Math.max(0, contents.optInt("phrases", 0)),
                    contents == null ? 0 : Math.max(0, contents.optInt("dictionary_words", 0)),
                    contents == null ? 0 : Math.max(0, contents.optInt("learning", 0))
                        + Math.max(0, contents.optInt("habits", 0)));
                if (compatibility != LocalBackupPolicy.Compatibility.OK) discard(archive);
                return new Prepared(archive, preview);
            }
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "backup preview failed", error);
            discard(archive);
            return null;
        }
    }

    /**
     * 恢复前把整个包校验一遍，全部通过才返回 true，这时才让用户确认恢复：
     *
     * <ol>
     *   <li>每个认识的条目按自己的上限读完、算 SHA-256，与 manifest 的 `checksums` 核对（{@link LocalBackupPolicy#checksumsMatch}；旧版本导出的包没有它，跳过这一步）；
     *   <li>设置、Android 本地设置、皮肤和常用语按恢复时的读法解析一遍；
     *   <li>词库快照和输入习惯解到工作目录，交给原生侧按各自的格式完整校验（`inspect_snapshot`、`inspect_habits`），校验完就删掉，恢复时再解一次。
     * </ol>
     *
     * 读文件本身出错（IOException）照常抛出，由调用方当作读不出这份文件。
     */
    private static boolean verify(Context context, ZipFile zip, JSONObject manifest, Path archive) throws IOException {
        Map<String, String> declared = null;
        JSONObject checksums = manifest.optJSONObject(LocalBackupPolicy.CHECKSUMS);
        if (manifest.has(LocalBackupPolicy.CHECKSUMS)) {
            if (checksums == null) return false;
            declared = new TreeMap<>();
            for (Iterator<String> names = checksums.keys(); names.hasNext(); ) {
                String name = names.next();
                if (!(checksums.opt(name) instanceof String digest)) return false;
                declared.put(name, digest);
            }
        }
        Map<String, String> actual = new TreeMap<>();
        for (String name : LocalBackupPolicy.ENTRIES) {
            ZipEntry entry = zip.getEntry(name);
            if (entry == null) continue;
            String digest = entryDigest(zip, entry, LocalBackupPolicy.entryLimit(name));
            if (digest == null) return false;
            actual.put(name, digest);
        }
        if (!LocalBackupPolicy.checksumsMatch(declared, actual)) {
            Log.w(TAG, "backup checksums do not match");
            return false;
        }
        try {
            JSONObject settings = jsonEntry(zip, LocalBackupPolicy.SETTINGS, LocalBackupPolicy.MAX_SETTINGS_BYTES);
            if (settings != null && settings.has("settings") && settings.optJSONObject("settings") == null) return false;
            jsonEntry(zip, LocalBackupPolicy.ANDROID_LOCAL, LocalBackupPolicy.MAX_ANDROID_LOCAL_BYTES);
            byte[] skins = entryBytes(zip, LocalBackupPolicy.SKINS, LocalBackupPolicy.MAX_SKINS_BYTES);
            if (skins != null) new JSONArray(TextPolicy.utf8(skins));
            JSONObject phrases = jsonEntry(zip, LocalBackupPolicy.PHRASES, LocalBackupPolicy.MAX_PHRASES_BYTES);
            if (phrases != null && phrases.has("phrases") && phrases.optJSONArray("phrases") == null) return false;
        } catch (JSONException malformed) {
            Log.w(TAG, "backup entry is not valid JSON", malformed);
            return false;
        }
        return inspectNative(context, zip, archive, LocalBackupPolicy.DICTIONARY, LocalBackupPolicy.MAX_DICTIONARY_BYTES,
                "inspect_snapshot")
            && inspectNative(context, zip, archive, LocalBackupPolicy.HABITS, LocalBackupPolicy.MAX_HABITS_BYTES,
                "inspect_habits");
    }

    /** 条目按 `limit` 读完算出的 SHA-256；超过上限时返回 null。 */
    @Nullable private static String entryDigest(ZipFile zip, ZipEntry entry, long limit) throws IOException {
        MessageDigest digest = sha256Digest();
        byte[] buffer = new byte[COPY_BUFFER];
        long total = 0;
        try (InputStream input = zip.getInputStream(entry)) {
            int count;
            while ((count = input.read(buffer)) != -1) {
                total += count;
                if (total > limit) return null;
                digest.update(buffer, 0, count);
            }
        }
        return DigestPolicy.hex(digest.digest());
    }

    /** 把条目 `name` 解到工作目录，让原生侧用 `operation` 只读校验；包里没有这个条目时算通过。校验完删掉解出来的文件。 */
    private static boolean inspectNative(Context context, ZipFile zip, Path archive, String name, long limit,
            String operation) throws IOException {
        ZipEntry entry = zip.getEntry(name);
        if (entry == null) return true;
        Path file = archive.resolveSibling("verify-" + name);
        try {
            try (InputStream input = zip.getInputStream(entry)) {
                copyBounded(input, file, limit);
            }
            CloudSync.nativeValue(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", operation)
                    .put("source", file.toAbsolutePath().toString())).toString()));
            return true;
        } catch (JSONException | RuntimeException rejected) {
            Log.w(TAG, "backup entry rejected: " + name, rejected);
            return false;
        } catch (IOException rejected) {
            // `nativeValue` 把原生侧的拒绝也报成 IOException；包本身读得出来，所以当作这一项坏了，而不是读不出整份文件。
            Log.w(TAG, "backup entry rejected: " + name, rejected);
            return false;
        } finally {
            deleteQuietly(file);
        }
    }

    static void discard(@Nullable Path archive) {
        deleteQuietly(archive);
    }

    /**
     * 从 {@link #prepare} 复制好、校验过的备份包恢复，返回给用户看的结果；恢复完删掉那份副本。
     *
     * <p>先恢复会改写本机现有内容的设置、皮肤和常用语：动手前用 {@link Undo#capture} 记下它们原来的样子，任何一部分写入失败就用 {@link Undo#apply} 全部换回去，不再碰词库和输入记录，整次恢复等于没有发生。这几部分都成功后才排词库、输入记录和输入习惯：它们只增不删、本机优先，排到一半失败时已经排进去的照样写入，在结果里点名没恢复的部分，再恢复一次不会重复。
     */
    static String restore(Context context, Path archive) {
        String directory = HostStore.directory(context);
        Path dictionary = archive.resolveSibling("restore-dictionary.ndjson");
        Path habits = archive.resolveSibling("restore-habits.ndjson");
        try {
            if (directory.isEmpty()) return "请先打开一次键盘完成首次准备，再来恢复。";
            try (ZipFile zip = new ZipFile(archive.toFile())) {
                Undo undo;
                try {
                    undo = Undo.capture(context, directory);
                } catch (IOException | JSONException | RuntimeException error) {
                    Log.w(TAG, "local state before restore unavailable", error);
                    return "读不出本机现在的设置，没有开始恢复，本机没有任何改动。请稍后重试。";
                }
                String part = "设置";
                boolean settings;
                int skins;
                int phrases;
                List<String> unheld = new ArrayList<>(1);
                try {
                    settings = restoreSettings(context, directory, zip);
                    part = "自定义皮肤";
                    skins = restoreSkins(directory, zip);
                    part = "常用语";
                    phrases = restorePhrases(context, zip, undo, unheld);
                } catch (IOException | JSONException | RuntimeException error) {
                    Log.w(TAG, part + " restore failed, undoing", error);
                    return LocalBackupPolicy.rolledBack(part, undo.apply(context, directory));
                }
                if (settings) SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                if (skins > 0) SyncSignals.markDirty(context, SyncSwitch.SKINS);
                adoptPresentStarters(context, undo.presentPhrases);
                List<String> failed = new ArrayList<>(4);
                failed.addAll(unheld);
                int[] words = restoreDictionary(context, zip, dictionary, failed);
                int habitCount = restoreHabits(context, zip, habits, failed);
                return LocalBackupPolicy.summary(new LocalBackupPolicy.Restored(settings, skins, phrases, words[0],
                    words[1], words[2], habitCount, failed));
            }
        } catch (IOException | RuntimeException error) {
            Log.w(TAG, "backup restore failed", error);
            return "读不出这份备份文件，请确认选的是导出的 .zip 文件。";
        } finally {
            deleteQuietly(dictionary);
            deleteQuietly(habits);
            deleteQuietly(archive);
        }
    }

    /**
     * 恢复前本机的设置、按键反馈、Android 本地设置和皮肤库，以及恢复过程中新加的常用语：恢复设置、皮肤或常用语时任何一部分失败，就用它把这几样换回恢复前的样子。
     *
     * <p>设置不按文件换回，而是把本机自己导出的设置文档（与云同步上传、本地备份导出的是同一份）按同样的方式再应用一次：偏好文件由 client-core 按修订号比较并交换写入，键盘进程同时在读写，直接把旧文件拷回去会让修订号倒退。Android 本地设置按键比对，只把变了的键改回原值（原来没写过的键删掉，回到默认值）。皮肤库整份写回（{@link CustomSkinLibrary#replaceAll}），常用语删掉这次新加的那几条。
     */
    private static final class Undo {
        final JSONObject settings;
        final KeyboardFeedbackStore.Settings feedback;
        final Map<String, Object> local;
        final List<CustomSkinLibrary.Item> skins;
        /** 这次恢复新加的常用语的 id，撤销时删掉。 */
        final List<String> addedPhrases = new ArrayList<>();
        /** 本机已经有的那几条正文；全部成功后才认领其中的预置示例（{@link #adoptPresentStarters}），失败时不用撤销认领。 */
        final List<String> presentPhrases = new ArrayList<>();

        private Undo(JSONObject settings, KeyboardFeedbackStore.Settings feedback, Map<String, Object> local,
                List<CustomSkinLibrary.Item> skins) {
            this.settings = settings;
            this.feedback = feedback;
            this.local = local;
            this.skins = skins;
        }

        static Undo capture(Context context, String directory) throws IOException, JSONException {
            JSONObject settings = CloudSync.nativeValue(NativeClient.accountSettingsExport(
                new JSONObject().put("preferences_directory", directory)
                    .put("feedback", CloudSync.hostFeedback(context)).toString()))
                .getJSONObject("settings");
            settings.remove(SyncMergePolicy.SKINS_KEY);
            return new Undo(settings, KeyboardFeedbackStore.load(context),
                new LinkedHashMap<>(AndroidLocalSettings.load(context).explicit()),
                CustomSkinLibrary.read(Paths.get(directory)));
        }

        /** 把设置、按键反馈、本地设置、皮肤和常用语换回恢复前；每一样都试，全部成功时返回 true。 */
        boolean apply(Context context, String directory) {
            boolean undone = true;
            for (String id : addedPhrases) {
                if (!CommonPhrasesStore.remove(context, id).ok()) undone = false;
            }
            try {
                CustomSkinLibrary.replaceAll(Paths.get(directory), skins);
            } catch (IOException | RuntimeException error) {
                Log.w(TAG, "skin undo failed", error);
                undone = false;
            }
            try {
                if (settings.length() > 0) applySettingsDocument(context, directory, settings);
            } catch (IOException | JSONException | RuntimeException error) {
                Log.w(TAG, "settings undo failed", error);
                undone = false;
            }
            try {
                KeyboardFeedbackStore.save(context, feedback);
            } catch (IOException | RuntimeException error) {
                Log.w(TAG, "keyboard feedback undo failed", error);
                undone = false;
            }
            try {
                Map<String, Object> current = AndroidLocalSettings.load(context).explicit();
                Map<String, Object> edits = new LinkedHashMap<>();
                Set<String> keys = new TreeSet<>(current.keySet());
                keys.addAll(local.keySet());
                for (String key : keys) {
                    if (!Objects.equals(current.get(key), local.get(key))) edits.put(key, local.get(key));
                }
                if (!edits.isEmpty()) AndroidLocalSettings.update(context, edits);
            } catch (IOException | RuntimeException error) {
                Log.w(TAG, "android local settings undo failed", error);
                undone = false;
            }
            return undone;
        }
    }

    /** 把一份设置文档按它自己每个值的类型声明字段表，交给 client-core 应用（与云同步同一条路）。偏好恰好被键盘抢先写了一次时重读重来一次。 */
    private static void applySettingsDocument(Context context, String directory, JSONObject settings)
            throws IOException, JSONException {
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
    }

    /** 设置文档按备份里的值声明字段类型后交给 client-core 应用，再写回 Android 本地设置。改了任何设置时返回 true；写入失败时抛出，由调用方撤销。 */
    private static boolean restoreSettings(Context context, String directory, ZipFile zip)
            throws IOException, JSONException {
        boolean applied = false;
        JSONObject document = jsonEntry(zip, LocalBackupPolicy.SETTINGS, LocalBackupPolicy.MAX_SETTINGS_BYTES);
        JSONObject settings = document == null ? null : document.optJSONObject("settings");
        if (settings != null && settings.length() > 0) {
            settings.remove(SyncMergePolicy.SKINS_KEY);
            applySettingsDocument(context, directory, settings);
            applied = true;
        }
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
                // 本地设置里有一部分随设置同步（应用主题、按键细节等），写回后同样标记待上传（由调用方在全部成功后标记）。
                AndroidLocalSettings.update(context, edits);
                applied = true;
            }
        }
        return applied;
    }

    /** 并入皮肤，返回新增或更新的个数；写入失败时抛出，由调用方撤销。 */
    private static int restoreSkins(String directory, ZipFile zip) throws IOException {
        byte[] bytes = entryBytes(zip, LocalBackupPolicy.SKINS, LocalBackupPolicy.MAX_SKINS_BYTES);
        if (bytes == null) return 0;
        return CustomSkinLibrary.importDesigns(Paths.get(directory), TextPolicy.utf8(bytes));
    }

    /**
     * 逐条加回常用语；本机已经有的跳过，记进 `undo.presentPhrases`，等全部成功后再认领其中的预置示例。返回新加的条数，新加的 id 记进 `undo` 以便撤销。正文不合规或超过上限的条目不算失败，计入 `unheld` 在结果里说明；读不出本机常用语或 client-core 报了别的错时抛出，由调用方撤销。
     */
    private static int restorePhrases(Context context, ZipFile zip, Undo undo, List<String> unheld)
            throws IOException, JSONException {
        JSONObject document = jsonEntry(zip, LocalBackupPolicy.PHRASES, LocalBackupPolicy.MAX_PHRASES_BYTES);
        JSONArray phrases = document == null ? null : document.optJSONArray("phrases");
        if (phrases == null) return 0;
        String duplicate = CommonPhrasesStore.failureMessage("common_phrases_duplicate");
        String invalid = CommonPhrasesStore.failureMessage("common_phrases_invalid");
        String full = CommonPhrasesStore.failureMessage("common_phrases_limit");
        String tooLarge = CommonPhrasesStore.failureMessage("common_phrases_too_large");
        int added = 0;
        int skipped = 0;
        for (int index = 0; index < phrases.length(); index++) {
            if (!(phrases.opt(index) instanceof String text) || !CommonPhrasesStore.validText(text)) {
                skipped++;
                continue;
            }
            CommonPhrasesStore.Result result = CommonPhrasesStore.add(context, text);
            if (result.ok()) {
                added++;
                String id = addedId(result.document(), text);
                if (id != null) undo.addedPhrases.add(id);
            } else if (duplicate.equals(result.failure())) {
                undo.presentPhrases.add(text);
            } else if (invalid.equals(result.failure()) || full.equals(result.failure())
                    || tooLarge.equals(result.failure())) {
                skipped++;
            } else {
                throw new IOException("common phrase not added: " + result.failure());
            }
        }
        if (skipped > 0) unheld.add(skipped + " 条常用语");
        return added;
    }

    /** 刚加进去的那条常用语的 id：自己添加的、正文相同的那一条。 */
    @Nullable private static String addedId(CommonPhrasesStore.Document document, String text) {
        for (CommonPhrasesStore.Phrase phrase : document.phrases()) {
            if (phrase.own() && phrase.text().equals(text)) return phrase.id();
        }
        return null;
    }

    /** 本机已有的同一段正文若是本机预置、还没被认领的示例（新手机打开过常用语就会放进去），把它认领成用户的常用语：在旧手机上它是用户自己的，不认领的话云同步会一直跳过它。认领失败最坏是这几条暂时不参与同步，和 {@link CommonPhrasesStore#add} 的处理相同。 */
    private static void adoptPresentStarters(Context context, List<String> present) {
        if (present.isEmpty()) return;
        try {
            CommonPhrasesStore.adoptStarters(context, present);
        } catch (IOException error) {
            Log.w(TAG, "starter record was not updated", error);
        }
    }

    /**
     * 个人词库和输入记录：解出快照。本机既没有用户词、也没有任何输入记录（新手机、重装，判断在 {@link LocalBackupPolicy#dictionaryRestore}）时把整份快照交给激活队列，词和输入记录原样恢复，与云同步第一次同步而本机没有词时的做法相同。整份激活会替换本机的全部学习状态，所以本机只要学过一点（哪怕还没有自造词）就不走这条路，改为合并：
     *
     * <ul>
     *   <li>词先全部记进命名词库的待发送队列（{@link DictionaryCollectionsStore#queueUnownedWords}），再在键盘空闲时一批批送进个人词库队列。个人词库队列同时只收 128 个未完成的请求，所以不能直接把几千个词塞进去（那样第 129 个以后的词全会被拒）。
     *   <li>输入记录交给 `queue_learning_merge` 挑出来存成待合并的文件，键盘收起后空闲时合并（本机已有的保留本机，选词计数取大）。改工作词库要独占维护权，键盘开着时设置页拿不到，所以不在这里直接合并。
     * </ul>
     *
     * 旧版本导出的备份里没有输入记录，只恢复词。返回 {恢复的词数, 跳过的词数, 输入记录条数}。
     */
    private static int[] restoreDictionary(Context context, ZipFile zip, Path file, List<String> failed) {
        try {
            ZipEntry entry = zip.getEntry(LocalBackupPolicy.DICTIONARY);
            if (entry == null) return new int[] {0, 0, 0};
            try (InputStream input = zip.getInputStream(entry)) {
                copyBounded(input, file, LocalBackupPolicy.MAX_DICTIONARY_BYTES);
            }
            List<SyncMergePolicy.Word> words = SyncApi.snapshotWords(file);
            LocalBackupPolicy.DictionaryRestore mode = words.isEmpty() ? LocalBackupPolicy.DictionaryRestore.MERGE
                : LocalBackupPolicy.dictionaryRestore(words.size(), localWordCount(context), localLearningCount(context));
            if (mode == LocalBackupPolicy.DictionaryRestore.ACTIVATE) {
                try {
                    CloudSync.enqueueDictionarySnapshot(context, file, DictionarySnapshotQueue.LOCAL_RESTORE_OWNER, 0);
                    SyncSignals.markDirty(context, SyncSwitch.DICTIONARY);
                    return new int[] {words.size(), 0, manifestLearning(zip)};
                } catch (IOException | DictionarySnapshotQueue.Failure unavailable) {
                    // 键盘还没打开过（没有发布本机词库版本）或者已有一份快照在排队：退回合并。
                    Log.i(TAG, "snapshot activation unavailable, merging instead", unavailable);
                }
            }
            int learning = queueLearning(context, file, failed);
            if (words.isEmpty()) return new int[] {0, 0, learning};
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
                return new int[] {queued, 0, learning};
            }
            return new int[] {queued, words.size() - queued, learning};
        } catch (IOException | RuntimeException error) {
            Log.w(TAG, "dictionary restore failed", error);
            failed.add("个人词库");
            return new int[] {0, 0, 0};
        }
    }

    /** 本机用户词数；读不出来时为 null，恢复改走合并（{@link LocalBackupPolicy#dictionaryRestore}）。 */
    @Nullable private static Integer localWordCount(Context context) {
        try {
            return CloudSync.userWordCount(context);
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "local word count unavailable, merging instead", error);
            return null;
        }
    }

    /** 本机日志里输入记录和输入习惯的条数之和（`learning_count` 的 `count` 加 `habits`），不含用户自己的词：整份激活会把两者都换掉。任何一个读不出来时为 null，恢复改走合并。 */
    @Nullable private static Integer localLearningCount(Context context) {
        try {
            JSONObject value = CloudSync.nativeValue(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", "learning_count")).toString()));
            Integer learning = DictionaryCollectionsStore.nonNegativeInteger(value.opt("count"));
            Integer habits = DictionaryCollectionsStore.nonNegativeInteger(value.opt("habits"));
            return learning == null || habits == null ? null : learning + habits;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "local input record count unavailable, merging instead", error);
            return null;
        }
    }

    /** 把快照里的输入记录排给键盘合并（`queue_learning_merge`），返回排进去的条数；快照里没有输入记录时返回 0。失败时在 `failed` 里记下「输入记录」，不影响词的恢复。 */
    private static int queueLearning(Context context, Path file, List<String> failed) {
        try {
            JSONObject value = CloudSync.nativeValue(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", "queue_learning_merge")
                    .put("source", file.toAbsolutePath().toString())).toString()));
            if (!JsonPolicy.strictTrue(value.opt("queued"))) return 0;
            Integer learning = DictionaryCollectionsStore.nonNegativeInteger(value.opt("learning"));
            return learning == null ? 0 : learning;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "input record restore failed", error);
            failed.add("输入记录");
            return 0;
        }
    }

    /**
     * 输入习惯（整句联想、连续选词、拼写纠错、自动纠错抑制、置顶）：解出条目，交给 `queue_habits_merge` 存成待合并的文件，键盘收起后空闲时与输入记录一起合并（计数取大，置顶保留本机）。整份激活另建的那一代没有这几张表，所以不管词库走哪条路都排它，空闲处理先激活再合并。旧版本导出的备份没有这个条目，返回 0。失败时在 `failed` 里记下「输入习惯」。
     */
    private static int restoreHabits(Context context, ZipFile zip, Path file, List<String> failed) {
        try {
            ZipEntry entry = zip.getEntry(LocalBackupPolicy.HABITS);
            if (entry == null) return 0;
            try (InputStream input = zip.getInputStream(entry)) {
                copyBounded(input, file, LocalBackupPolicy.MAX_HABITS_BYTES);
            }
            JSONObject value = CloudSync.nativeValue(NativeClient.dictionary(new JSONObject()
                .put("options", new JSONObject(CloudSync.hostOptions(context)))
                .put("action", new JSONObject().put("operation", "queue_habits_merge")
                    .put("source", file.toAbsolutePath().toString())).toString()));
            if (!JsonPolicy.strictTrue(value.opt("queued"))) return 0;
            Integer habits = DictionaryCollectionsStore.nonNegativeInteger(value.opt("habits"));
            return habits == null ? 0 : habits;
        } catch (IOException | JSONException | RuntimeException error) {
            Log.w(TAG, "input habit restore failed", error);
            failed.add("输入习惯");
            return 0;
        }
    }

    /** manifest 里记的输入记录条数，只用来在整份激活时告诉用户恢复了多少；读不出来时为 0。 */
    private static int manifestLearning(ZipFile zip) {
        try {
            JSONObject manifest = jsonEntry(zip, LocalBackupPolicy.MANIFEST, LocalBackupPolicy.MAX_MANIFEST_BYTES);
            JSONObject contents = manifest == null ? null : manifest.optJSONObject("contents");
            return contents == null ? 0 : Math.max(0, contents.optInt("learning", 0));
        } catch (IOException | JSONException error) {
            return 0;
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
