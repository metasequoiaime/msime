package app.msime.android;

import android.content.Context;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import org.json.JSONArray;
import org.json.JSONObject;

/** 首次安装时解包词库并准备配置。已有配置只经共享刷新跟上安装包：可选的离线释义、辅助码表与语言词库随安装包替换；安装包换了词库版本时，把 APK 里的词库重新解包到同一个资源目录，再由刷新准备新代次。 */
public final class Bootstrap {
    private Bootstrap() {}
    public static boolean prepare(Context context) throws Exception {
        File root = context.getFilesDir();
        try (FileChannel channel = openLock(new File(root, "bootstrap.lock").toPath());
             FileLock lock = channel.lock()) {
            if (!lock.isValid()) throw new IllegalStateException("Bootstrap lock unavailable");
            installOfflineGlosses(context, new File(root, "bootstrap/offline-glosses"));
            installHelpcodes(context, new File(root, "bootstrap/resources/helpcodes"));
            // Before the configuration exists, so that prepare_host below finds them beside the resources and records them.
            installLanguageDictionaries(context, new File(root, "bootstrap/language-dictionaries"));
            installSoundPacks(context, new File(root, "sound-packs"));
            File configuration = new File(root, "runtime-options.json");
            File resources = new File(root, "bootstrap/resources");
            if (existingConfiguration(configuration)) {
                refreshExistingConfiguration(context, configuration, resources);
                return false;
            }
            extractDictionary(context, resources);
            JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
            // 不是 full 的版本把版本 id 交给 host-api：它按本版本的资源锁校验 APK 里的词库，在状态目录记下版本，从此没有偏好文件时读到的就是本版本的默认偏好（五笔版默认五笔、混拼打开）。full 不带这个键，请求与引入版本之前相同。
            AppEdition edition = AppEdition.current();
            if (!edition.isFull()) request.put("edition", edition.id());
            JSONObject result = new JSONObject(NativeClient.prepareHost(request.toString()));
            if (!result.getBoolean("ok")) throw new IllegalStateException("Shared resource verification/preparation failed: " + result.optString("error"));
            AtomicFile destination = new AtomicFile(configuration);
            FileOutputStream output = null;
            try {
                output = destination.startWrite();
                output.write(result.getJSONObject("value").toString().getBytes(StandardCharsets.UTF_8));
                destination.finishWrite(output);
            } catch (Exception error) {
                if (output != null) destination.failWrite(output);
                throw error;
            }
            return true;
        }
    }

    static FileChannel openLock(java.nio.file.Path path) throws java.io.IOException {
        if (Files.isSymbolicLink(path)
                || (Files.exists(path, LinkOption.NOFOLLOW_LINKS)
                    && !Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)))
            throw new java.io.IOException("Bootstrap lock is not a regular file");
        return FileChannel.open(path, StandardOpenOption.CREATE, StandardOpenOption.WRITE,
            LinkOption.NOFOLLOW_LINKS);
    }

    static boolean existingConfiguration(File file) throws java.io.IOException {
        if (file == null) throw new java.io.IOException("Runtime options unavailable");
        java.nio.file.Path path = file.toPath();
        if (Files.isSymbolicLink(path)
                || (Files.exists(path, LinkOption.NOFOLLOW_LINKS)
                    && !Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)))
            throw new java.io.IOException("Runtime options are not a regular file");
        return Files.exists(path, LinkOption.NOFOLLOW_LINKS);
    }

    static void ensureSafeDirectory(java.nio.file.Path directory) throws java.io.IOException {
        SafePaths.ensureDirectory(directory);
    }

    /**
     * Non-English candidate glosses (scripts/build_offline_glosses.py), extracted beside the resources where the Engine looks for one zh-&lt;lang&gt;.db per target language.
     *
     * <p>Unlike the dictionary they are not part of the verified configuration, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left. A failure leaves the keyboard glossing in English only, never without an Engine.
     */
    private static void installOfflineGlosses(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "offline-glosses.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] names = context.getAssets().list("offline-glosses");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("offline-glosses/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Offline gloss extraction failed", error);
        }
    }

    /**
     * The helpcode tables (resources/helpcodes), extracted into helpcodes/ under the resource directory, where the Engine reads them.
     *
     * <p>Like the offline glosses they are not part of the verified dictionary (the shared verification lets a real helpcodes/ directory through), so they follow the installed package: an install prepared before they shipped gets them on the first start after the update, although its configuration is never rewritten. Each packaged file is replaced on its own through a temporary sibling and an atomic rename, so a table being read is never half written and the user's own tables under helpcodes/custom are left alone. A failure leaves helpcode input narrowing nothing, never the keyboard without an Engine.
     */
    private static void installHelpcodes(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            ensureSafeDirectory(destination.toPath());
            String[] names = context.getAssets().list("helpcodes");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("helpcodes/" + name)) {
                    copyAsset(input, new File(destination, name).toPath());
                }
            }
            writeAtomically(marker.toPath(), stamp.getBytes(StandardCharsets.UTF_8));
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Helpcode table extraction failed", error);
        }
    }

    /**
     * The Cantonese, Zhuyin and Stroke dictionaries (scripts/fetch_language_dictionaries.py), extracted to language-dictionaries/ beside the resources, where host-api looks for `msime-cantonese.db`, `msime-zhuyin.db` and `msime-stroke.db` and names the directory in the runtime options.
     *
     * <p>Like the offline glosses they are not part of the verified dictionary, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left, which takes those schemes off the keyboard once the configuration is refreshed. The directory is swapped whole through a staging sibling and an atomic rename. A failure leaves Cantonese, Zhuyin and Stroke unavailable, never the keyboard without an Engine.
     */
    private static void installLanguageDictionaries(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "language-dictionaries.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] names = context.getAssets().list("language-dictionaries");
            for (String name : names == null ? new String[0] : names) {
                if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                try (InputStream input = context.getAssets().open("language-dictionaries/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Language dictionary extraction failed", error);
        }
    }

    /**
     * 内置按键音包（resources/sound-packs 里 `mode = "keys"` 的包，连同各自的 plugin.toml 许可信息），解到 `<filesDir>/sound-packs/<id>/`，键盘经 `NativeClient.keySoundPack` 校验后用 SoundPool 播放其中的样本。
     *
     * <p>与离线释义、语言词库同样的规则：不属于校验过的词库，跟着安装包走；安装包变了就经同级的暂存目录整体替换，旧包留下的目录一起去掉。失败时键盘只用系统按键音。
     */
    private static void installSoundPacks(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "sound-packs.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] packs = context.getAssets().list("sound-packs");
            for (String pack : packs == null ? new String[0] : packs) {
                if (!pack.matches("[A-Za-z0-9_.-]+") || pack.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                String[] names = context.getAssets().list("sound-packs/" + pack);
                if (names == null || names.length == 0) continue;
                File directory = new File(staging, pack);
                ensureSafeDirectory(directory.toPath());
                for (String name : names) {
                    if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
                    try (InputStream input = context.getAssets().open("sound-packs/" + pack + "/" + name)) {
                        copyAsset(input, new File(directory, name).toPath());
                    }
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Sound pack extraction failed", error);
        }
    }

    /**
     * 把 APK 里 `desktop-dictionary.lock.json` 固定的词库解包到 `resources`。共享校验要求资源目录恰好是锁里的文件（外加 `helpcodes/`），所以先删掉锁里没有的条目（例如统一 `msime-` 前缀之前的旧文件名），`helpcodes/` 连同用户自己的辅助码表原样保留。每个文件经临时同级文件原子替换，中途失败时下次启动的刷新仍报词库过期，会再解包一次。
     */
    private static void extractDictionary(Context context, File resources) throws Exception {
        ensureSafeDirectory(resources.toPath());
        JSONObject manifest;
        // 各版本的 APK 都把本版本的资源锁放在这个文件名下（build-apk.sh 选的；full 的就是 resources/desktop-dictionary.lock.json 本身），下面只解出锁里列的文件。
        try (InputStream input = context.getAssets().open("desktop-dictionary.lock.json")) {
            // Small immutable APK manifest; large dictionary files are streamed below.
            java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > 16384) throw new IllegalArgumentException("Manifest too large");
                bytes.write(buffer, 0, count);
            }
            manifest = new JSONObject(bytes.toString(StandardCharsets.UTF_8.name()));
        }
        JSONArray artifacts = manifest.getJSONArray("artifacts");
        java.util.Set<String> names = new java.util.HashSet<>();
        for (int index = 0; index < artifacts.length(); index++) {
            String name = artifacts.getJSONObject(index).getString("name");
            if (!name.matches("[A-Za-z0-9_.-]+") || name.contains("..")) throw new IllegalArgumentException("Invalid asset name");
            names.add(name);
        }
        File[] existing = resources.listFiles();
        for (File entry : existing == null ? new File[0] : existing) {
            if (entry.getName().equals("helpcodes") || names.contains(entry.getName())) continue;
            deleteTree(entry);
        }
        for (String name : names) {
            try (InputStream input = context.getAssets().open("dictionary/" + name)) {
                copyAsset(input, new File(resources, name).toPath());
            }
        }
    }

    /**
     * 经共享的 `msime_client_refresh_host` 让已有配置跟上安装包：`language_dictionaries` 对齐资源目录旁实际安装的语言词库，工作词库属于旧代次时准备新代次。输入法与这里的准备来自同一个安装包，不会留下读不懂新键的旧宿主。
     *
     * <p>安装包换了词库版本时，资源目录里还是旧版本解包出的文件，刷新报 `dictionary_outdated` 并保持配置不变。配置记录的就是本宿主自己的 `bootstrap/resources` 时，重新解包后再刷新一次，由 Engine 复制新代次并回放用户词库日志；其他目录不碰。失败只记一条不含路径的日志，输入法继续用原来的代次。
     */
    private static void refreshExistingConfiguration(Context context, File configuration, File resources) {
        try {
            String error = refreshHost(configuration);
            if (error == null || !error.startsWith("dictionary_outdated")) return;
            String recorded = readConfiguredResources(configuration);
            if (recorded == null || !new File(recorded).getCanonicalPath().equals(resources.getCanonicalPath())) return;
            extractDictionary(context, resources);
            refreshHost(configuration);
        } catch (Exception | LinkageError error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed", error);
        }
    }

    /** 刷新一次配置；成功时返回 `null`，失败时记日志并返回共享层的错误文本。 */
    private static String refreshHost(File configuration) throws Exception {
        JSONObject result = new JSONObject(NativeClient.refreshHost(configuration.getAbsolutePath()));
        if (result.optBoolean("ok")) return null;
        String error = result.optString("error");
        android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed: "
            + (error.startsWith("dictionary_outdated") ? "dictionary_outdated" : "error"));
        return error;
    }

    /** 配置里记录的 `resources`；超过 1 MiB 或读不出来时为 `null`。按块读并限长，文件在检查之后变大也不会无界分配。 */
    private static String readConfiguredResources(File configuration) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream(8192);
        try (InputStream input = Files.newInputStream(configuration.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() > 1024 * 1024 - count) return null;
                bytes.write(buffer, 0, count);
            }
        }
        String resources = new JSONObject(bytes.toString(StandardCharsets.UTF_8.name())).optString("resources", "");
        return resources.isEmpty() ? null : resources;
    }

    static String readMarker(java.nio.file.Path file) {
        if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) return null;
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(64);
            byte[] buffer = new byte[64];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() > 64 - count) return null;
                bytes.write(buffer, 0, count);
            }
            return bytes.toString(StandardCharsets.UTF_8.name());
        } catch (Exception ignored) {
            return null;
        }
    }

    /** Copy through a newly-created sibling so an existing destination link is replaced, never followed. */
    static void copyAsset(InputStream input, java.nio.file.Path destination) throws IOException {
        java.nio.file.Path parent = destination.getParent();
        if (parent == null) throw new IOException("asset destination unavailable");
        java.nio.file.Path temporary = Files.createTempFile(parent,
            "." + destination.getFileName() + ".", ".staging");
        try {
            Files.copy(input, temporary, StandardCopyOption.REPLACE_EXISTING);
            Files.move(temporary, destination, StandardCopyOption.REPLACE_EXISTING,
                StandardCopyOption.ATOMIC_MOVE);
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    static void writeAtomically(java.nio.file.Path destination, byte[] bytes) throws IOException {
        try (InputStream input = new java.io.ByteArrayInputStream(bytes)) {
            copyAsset(input, destination);
        }
    }

    private static void deleteTree(File file) throws java.io.IOException {
        java.nio.file.Path path = file.toPath();
        if (Files.isSymbolicLink(path)) {
            Files.deleteIfExists(path);
            return;
        }
        if (Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS)) {
            try (java.nio.file.DirectoryStream<java.nio.file.Path> children = Files.newDirectoryStream(path)) {
                for (java.nio.file.Path child : children) deleteTree(child.toFile());
            }
        }
        Files.deleteIfExists(path);
    }
}
