package app.msime.android;

import app.msime.android.policy.HostOptionsPolicy;
import android.content.Context;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
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

/** 首次安装时解包词库并准备配置。已有配置只经共享刷新跟上安装包：可选的离线释义、辅助码表、语言词库与背单词内置词书随安装包替换；安装包换了词库版本时，把 APK 里的词库重新解包到同一个资源目录，再由刷新准备新代次。 */
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
            installSoundPacks(context, HostOptionsPolicy.soundPacksDirectory(root));
            installWordbooks(context, new File(root, "bootstrap/wordbooks"));
            File configuration = new File(root, "runtime-options.json");
            File resources = new File(root, "bootstrap/resources");
            // 必须在刷新和 extractDictionary 之前：extractDictionary 会把新安装包的锁里没有的文件移出资源目录，老用户已解压的日文词典和五笔码表要先收编。
            adoptOnDemandGroups(context, root, resources);
            if (existingConfiguration(configuration)) {
                refreshExistingConfiguration(context, configuration, resources);
                return false;
            }
            extractDictionary(context, resources);
            clearInterruptedStaging(new File(root, "bootstrap/state/user/dictionaries"));
            JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                .put("state_root", new File(root, "bootstrap/state").getAbsolutePath());
            // 不是 full 的版本把版本 id 交给 host-api：它按本版本的资源锁校验 APK 里的词库，在状态目录记下版本，从此没有偏好文件时读到的就是本版本的默认偏好（五笔版默认五笔、混拼打开）。full 不带这个键，请求与引入版本之前相同。
            AppEdition edition = AppEdition.current();
            if (!edition.isFull()) request.put("edition", edition.id());
            JSONObject result = new JSONObject(NativeClient.prepareHost(request.toString()));
            if (!JsonPolicy.strictTrue(result.opt("ok"))) {
                throw new IllegalStateException("Shared resource verification/preparation failed: "
                    + JsonPolicy.strictStringOrEmpty(result.opt("error")));
            }
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

    /**
     * 删掉上一次首次准备被打断时留下的 `*.incoming` 暂存目录。
     *
     * <p>引擎每次准备代次都独占地新建一个 `<content id>.<随机串>.incoming`，失败会自己删掉，但进程在复制约 190 MB 词库的途中被杀时它就留下了。首启引导恰好在这几秒里把用户送去系统设置启用键盘，小米等系统会把退到后台的应用杀掉。留下的目录不再挡住下一次准备，引擎也会在它一小时没被写过之后自己删掉（`generation.rs` 的 `sweep_abandoned_stages`）；这里在首次准备前立刻删掉，不让半截词库多占一小时空间。这里只在还没有配置时调用，并且持有 `bootstrap.lock`：没有配置就没有会话，不会有别的进程正在暂存。
     */
    static void clearInterruptedStaging(File dictionaries) throws java.io.IOException {
        File[] entries = dictionaries.listFiles();
        for (File entry : entries == null ? new File[0] : entries) {
            if (entry.getName().endsWith(".incoming")) deleteTree(entry);
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
        if (!privateConfiguration(path))
            throw new java.io.IOException("Runtime options are not a regular file");
        return Files.exists(path, LinkOption.NOFOLLOW_LINKS);
    }

    static boolean privateConfiguration(java.nio.file.Path path) {
        return path != null
            && (!Files.exists(path, LinkOption.NOFOLLOW_LINKS)
                || (Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS)
                    && SafePaths.isSingleLink(path)));
    }

    static void ensureSafeDirectory(java.nio.file.Path directory) throws java.io.IOException {
        SafePaths.ensureDirectory(directory);
    }

    /** Only a single ASCII asset component may be appended beneath a bootstrap directory. */
    static void requireAssetName(String name) {
        if (name == null || !name.matches("[A-Za-z0-9_.-]+") || name.contains(".."))
            throw new IllegalArgumentException("Invalid asset name");
    }

    /**
     * Non-English candidate glosses (scripts/build_offline_glosses.py), extracted beside the resources where the Engine looks for one zh-&lt;lang&gt;.db per target language.
     *
     * <p>Unlike the dictionary they are not part of the verified configuration, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left. 不再随包带它们的安装包（full 版改为按需下载 offline-glosses 资源包）替换目录之前，先把上一个安装包留下的那组收编为资源包，收编不了才删。A failure leaves the keyboard glossing in English only, never without an Engine.
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
                requireAssetName(name);
                try (InputStream input = context.getAssets().open("offline-glosses/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            adoptPrevious(context.getFilesDir(), staging, destination, ResourcePacks.OFFLINE_GLOSSES);
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
                requireAssetName(name);
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
     * <p>Like the offline glosses they are not part of the verified dictionary, so they follow the installed package: an update replaces them, and a package built without them removes any an earlier one left, which takes those schemes off the keyboard once the configuration is refreshed. 不再随包带它们的安装包（full 版改为按需下载 language-dictionaries 资源包）替换目录之前，先把上一个安装包留下的那组收编为资源包，host-api 从资源包里找到它们，收编不了才删。The directory is swapped whole through a staging sibling and an atomic rename. A failure leaves Cantonese, Zhuyin and Stroke unavailable, never the keyboard without an Engine.
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
                requireAssetName(name);
                try (InputStream input = context.getAssets().open("language-dictionaries/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            adoptPrevious(context.getFilesDir(), staging, destination, ResourcePacks.LANGUAGE_DICTIONARIES);
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Language dictionary extraction failed", error);
        }
    }

    /**
     * 背单词的八本内置词书（scripts/fetch_wordbooks.py 从 ECDICT 生成）和它们的许可声明，解到 `bootstrap/wordbooks`，与 `bootstrap/resources` 同级：设置页把 `bootstrap` 作为资源根交给共享层，共享层在它下面的 `wordbooks/` 里找 `<id>.json`（`builtin::directory`）。
     *
     * <p>与语言词库同样的规则：不属于校验过的词库，跟着安装包走；安装包变了就经同级的暂存目录整体替换，不再带词书的安装包会把旧的一并去掉。词书不是按需资源包，没有收编这一步。失败时背单词只列用户导入的词表，键盘和词库准备都不受影响。
     */
    private static void installWordbooks(Context context, File destination) {
        try {
            String stamp = Long.toString(context.getPackageManager()
                .getPackageInfo(context.getPackageName(), 0).lastUpdateTime);
            File marker = new File(destination, ".package");
            if (Files.isRegularFile(marker.toPath(), LinkOption.NOFOLLOW_LINKS)
                    && stamp.equals(readMarker(marker.toPath()))) return;
            File staging = new File(destination.getParentFile(), "wordbooks.staging");
            ensureSafeDirectory(destination.getParentFile().toPath());
            deleteTree(staging);
            ensureSafeDirectory(staging.toPath());
            String[] names = context.getAssets().list("wordbooks");
            for (String name : names == null ? new String[0] : names) {
                requireAssetName(name);
                try (InputStream input = context.getAssets().open("wordbooks/" + name)) {
                    copyAsset(input, new File(staging, name).toPath());
                }
            }
            writeAtomically(new File(staging, ".package").toPath(), stamp.getBytes(StandardCharsets.UTF_8));
            deleteTree(destination);
            Files.move(staging.toPath(), destination.toPath(), StandardCopyOption.ATOMIC_MOVE);
        } catch (Exception error) {
            // 只记失败的类别，不带异常消息：消息里可能有应用私有目录的路径。
            android.util.Log.w("MSIMEBootstrap", "Wordbook extraction failed: " + error.getClass().getSimpleName());
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
                requireAssetName(pack);
                String[] names = context.getAssets().list("sound-packs/" + pack);
                if (names == null || names.length == 0) continue;
                File directory = new File(staging, pack);
                ensureSafeDirectory(directory.toPath());
                for (String name : names) {
                    requireAssetName(name);
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
     * 把 APK 里 `desktop-dictionary.lock.json` 固定的词库解包到 `resources`。共享校验要求资源目录恰好是锁里的文件（外加 `helpcodes/`），所以先删掉锁里没有的条目（例如统一 `msime-` 前缀之前的旧文件名），`helpcodes/` 连同用户自己的辅助码表原样保留。例外是收编没成的按需文件组（日文词典那一组或五笔码表，{@link #adoptOnDemandGroups} 遇到暂时性失败时留下的）：改名挪进这一组的 {@link #pending} 目录等下次启动再收编，不删，免得用户重新下载 66 MB 的日文词典或近百 MB 的五笔码表。每个文件经临时同级文件原子替换，中途失败时下次启动的刷新仍报词库过期，会再解包一次。
     */
    private static void extractDictionary(Context context, File resources) throws Exception {
        ensureSafeDirectory(resources.toPath());
        java.util.Set<String> names = packagedArtifacts(context);
        File[] existing = resources.listFiles();
        for (File entry : existing == null ? new File[0] : existing) {
            if (entry.getName().equals("helpcodes") || names.contains(entry.getName())) continue;
            OnDemandGroup group = onDemandGroupOf(entry.getName());
            if (group != null && Files.isRegularFile(entry.toPath(), LinkOption.NOFOLLOW_LINKS)) {
                File pending = pending(resources, group);
                ensureSafeDirectory(pending.toPath());
                Files.move(entry.toPath(), new File(pending, entry.getName()).toPath(),
                    StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
                continue;
            }
            deleteTree(entry);
        }
        for (String name : names) {
            try (InputStream input = context.getAssets().open("dictionary/" + name)) {
                copyAsset(input, new File(resources, name).toPath());
            }
        }
    }

    /** 本安装包随带的词库文件名：APK 里 `desktop-dictionary.lock.json` 列出的那些。 */
    private static java.util.Set<String> packagedArtifacts(Context context) throws Exception {
        JSONObject manifest;
        // 各版本的 APK 都把本版本的资源锁放在这个文件名下（build-apk.sh 选的；full 的就是 resources/desktop-dictionary.lock.json 本身，不随包的按需资源已从中剔除），只解出锁里列的文件。
        try (InputStream input = context.getAssets().open("desktop-dictionary.lock.json")) {
            // Small immutable APK manifest; large dictionary files are streamed by the caller.
            byte[] bytes = HttpBodyPolicy.readRequired(input, 16384);
            manifest = new JSONObject(new String(bytes, StandardCharsets.UTF_8));
        }
        JSONArray artifacts = manifest.getJSONArray("artifacts");
        java.util.Set<String> names = new java.util.HashSet<>(artifacts.length());
        for (int index = 0; index < artifacts.length(); index++) {
            String name = artifacts.getJSONObject(index).getString("name");
            requireAssetName(name);
            names.add(name);
        }
        return names;
    }

    /**
     * 发布包可以不带、改为按需下载的一组词库文件，与 crates/client-core/src/resources.rs 的 `on_demand_artifacts("android")` 一致：共享校验要求每组整组在或整组不在资源目录里。
     *
     * @param pack 这组文件收编成的资源包
     * @param pending 收编没成、又被 {@link #extractDictionary} 挪出资源目录时暂放的目录名，与资源目录同级、在同一个文件系统上，下次启动从那里收编
     */
    private record OnDemandGroup(String pack, java.util.List<String> artifacts, String pending) {}

    /** 日文词典与它的两份 Mozc 许可文本（`ON_DEMAND_JAPANESE_ARTIFACTS`）。 */
    private static final OnDemandGroup JAPANESE_GROUP = new OnDemandGroup(ResourcePacks.JAPANESE,
        java.util.List.of("msime-japanese.dat", "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt"),
        "japanese-pending");
    /** 五笔码表（`ON_DEMAND_WUBI_ARTIFACTS`）。full 和拼音版的瘦包不带它，五笔版照旧随包。 */
    private static final OnDemandGroup WUBI_GROUP = new OnDemandGroup(ResourcePacks.WUBI,
        java.util.List.of("msime-wubi.db"), "wubi-pending");
    private static final java.util.List<OnDemandGroup> ON_DEMAND_GROUPS = java.util.List.of(JAPANESE_GROUP, WUBI_GROUP);

    /** 文件 `name` 属于哪一组按需文件；都不属于时为 null。 */
    private static OnDemandGroup onDemandGroupOf(String name) {
        for (OnDemandGroup group : ON_DEMAND_GROUPS) if (group.artifacts().contains(name)) return group;
        return null;
    }

    /** 收编没成、又被 {@link #extractDictionary} 挪出资源目录的那组文件暂放在这里（例如 `bootstrap/japanese-pending`）。 */
    private static File pending(File resources, OnDemandGroup group) {
        return new File(resources.getParentFile(), group.pending());
    }

    /** 资源包 `pack` 的那组文件是否正在 pending 目录里等下次启动收编（{@link #adoptOnDemandGroups} 上次遇到暂时性失败）。这时本机其实有这份字节，只是资源目录和资源包里都看不到它，不要为它联网下载。`files` 是 {@link Context#getFilesDir()}。 */
    public static boolean adoptionPending(File files, String pack) {
        for (OnDemandGroup group : ON_DEMAND_GROUPS) {
            if (!group.pack().equals(pack)) continue;
            File pending = pending(new File(files, "bootstrap/resources"), group);
            for (String name : group.artifacts()) {
                if (Files.isRegularFile(new File(pending, name).toPath(), LinkOption.NOFOLLOW_LINKS)) return true;
            }
        }
        return false;
    }

    /**
     * 升级时把上一个安装包解压在 `bootstrap/resources` 里的按需文件组（日文词典那一组、五笔码表）收编为资源包，不重新下载。
     *
     * <p>一组只在本安装包的锁里已经没有它时处理：日文版、五笔版和仍随包带着它的安装包照旧从资源目录读。先收编资源目录里的那组，再收编 {@link #pending} 里的那组。收编是同一文件系统内的改名，不复制；输入法进程正内存映射着的文件 inode 不变，映射继续有效，下次获得焦点时 host-api 改从资源包读。收编成功（或同一组字节早已装好）后来源目录里不再留这组文件，共享校验按「整组缺席」通过。字节对不上（例如安装包同时换了词库版本，旧版本的五笔码表与新锁不符）或这组不完整时整组删掉，等用户需要时再按需下载；其他失败（例如另一个下载正占着这个资源包）原样保留，下次启动再试，那之前资源目录里仍是完整的一组。失败只记一条不含路径的日志，不影响准备。
     *
     * <p>本版本不提供五笔（拼音版）时五笔码表不收编、直接删掉：拼音版以前随包带着它却从来用不到，收编成资源包只会让词库自动更新每次都替它下载新版本。
     *
     * <p>进程在收编途中被杀时，这组文件（或其中几个）留在共享层的暂存目录里，资源目录里一个也不剩或只剩几个。所以资源包还没装好时，资源目录里没有这组文件也照样调用一次收编：共享层先按暂存目录里的来源记录把文件放回原处，再照常收编。
     */
    private static void adoptOnDemandGroups(Context context, File root, File resources) {
        java.util.Set<String> packaged;
        try {
            packaged = packagedArtifacts(context);
        } catch (Exception | LinkageError error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "On-demand dictionary adoption failed", error);
            return;
        }
        for (OnDemandGroup group : ON_DEMAND_GROUPS) {
            try {
                if (packaged.contains(group.artifacts().get(0))) continue;
                if (group == WUBI_GROUP && !AppEdition.current().offers("wubi")) {
                    for (String name : group.artifacts()) {
                        deleteTree(new File(resources, name));
                        deleteTree(new File(pending(resources, group), name));
                    }
                    continue;
                }
                adoptFrom(root, resources, group, !ResourcePacks.installed(root, group.pack()));
                adoptFrom(root, pending(resources, group), group, false);
            } catch (Exception | LinkageError error) {
                android.util.Log.w("MSIMEBootstrap", "On-demand dictionary adoption failed: " + group.pack(), error);
            }
        }
    }

    /** 收编 `directory` 里的那组文件；`directory` 里一个也没有时，只有 `recover` 为真才调用（让共享层放回被打断的收编移走的文件）。 */
    private static void adoptFrom(File root, File directory, OnDemandGroup group, boolean recover) throws IOException {
        boolean present = false;
        for (String name : group.artifacts()) {
            present |= Files.exists(new File(directory, name).toPath(), LinkOption.NOFOLLOW_LINKS);
        }
        if (!present && !recover) return;
        if (!Files.isDirectory(directory.toPath(), LinkOption.NOFOLLOW_LINKS)) return;
        try {
            ResourcePacks.adopt(root, group.pack(), directory);
        } catch (ResourcePacks.Failure failure) {
            // 本来就没有这组文件时缺文件是常态，不记日志。
            if (present || !"local_model_missing_file".equals(failure.code()))
                android.util.Log.w("MSIMEBootstrap", "On-demand dictionary adoption failed: " + group.pack() + " " + failure.code());
            if (!unusableForAdoption(failure)) return;
        }
        for (String name : group.artifacts()) deleteTree(new File(directory, name));
    }

    /**
     * 新安装包不再带某一组可选文件（目录里没有任何 `.db`），而上一个安装包解压的那组还在 `previous` 里时，先把它收编为资源包 `pack`，再由调用方整体替换目录。收编失败时什么也不做，调用方照常删掉旧目录，这组改为按需下载。
     *
     * <p>上一次收编在途中被杀时，`previous` 里的 `.db` 已经挪进共享层的暂存目录，而 `.package` 标记还是旧的，所以这一步会再走一次。资源包还没装好时即使 `previous` 里没有 `.db` 也调用一次收编，共享层先把那些文件放回 `previous` 再照常收编。
     */
    private static void adoptPrevious(File root, File staging, File previous, String pack) {
        if (containsDatabase(staging)) return;
        boolean present = containsDatabase(previous);
        if (!present && (ResourcePacks.installed(root, pack)
                || !Files.isDirectory(previous.toPath(), LinkOption.NOFOLLOW_LINKS))) return;
        try {
            ResourcePacks.adopt(root, pack, previous);
        } catch (ResourcePacks.Failure failure) {
            if (present || !"local_model_missing_file".equals(failure.code()))
                android.util.Log.w("MSIMEBootstrap", "Resource pack adoption failed: " + pack + " " + failure.code());
        }
    }

    private static boolean containsDatabase(File directory) {
        File[] entries = directory.listFiles();
        for (File entry : entries == null ? new File[0] : entries) {
            if (entry.getName().endsWith(".db")
                    && Files.isRegularFile(entry.toPath(), LinkOption.NOFOLLOW_LINKS)) return true;
        }
        return false;
    }

    /** 这些失败说明本机那组文件本身不能用（字节不对、缺文件或是符号链接），留着也没用。 */
    private static boolean unusableForAdoption(ResourcePacks.Failure failure) {
        return switch (failure.code()) {
            case "local_model_size_mismatch", "local_model_checksum_mismatch",
                 "local_model_missing_file", "local_model_unsafe_archive" -> true;
            default -> false;
        };
    }

    /**
     * 经共享的 `msime_client_refresh_host` 让已有配置跟上安装包：`language_dictionaries` 对齐资源目录旁实际安装的语言词库，工作词库属于旧代次时准备新代次。输入法与这里的准备来自同一个安装包，不会留下读不懂新键的旧宿主。
     *
     * <p>安装包换了词库版本时，资源目录里还是旧版本解包出的文件，刷新报 `dictionary_outdated` 并保持配置不变。配置记录的就是本宿主自己的 `bootstrap/resources` 时，重新解包后再刷新一次，由 Engine 复制新代次并回放用户词库日志；其他目录不碰。失败只记一条不含路径的日志，输入法继续用原来的代次。
     */
    private static void refreshExistingConfiguration(Context context, File configuration, File resources) {
        try {
            JSONObject options = readConfiguredOptions(configuration);
            String recorded = options == null ? null : JsonPolicy.strictStringOrEmpty(options.opt("resources"));
            boolean bundledDirectory = recorded != null
                && new File(recorded).getCanonicalPath().equals(resources.getCanonicalPath());
            File state = ResourcePacks.stateRoot(context.getFilesDir());
            boolean ownedState = options != null && state.getCanonicalFile().equals(
                new File(JsonPolicy.strictStringOrEmpty(options.opt("preferences_directory"))).getCanonicalFile());
            File updates = new File(state, "dictionary-updates/generations").getCanonicalFile();
            boolean updatedDirectory = ownedState && recorded != null
                && new File(recorded).getCanonicalFile().toPath().startsWith(updates.toPath());
            if (bundledDirectory || updatedDirectory) {
                restorePackagedOnDemandGroups(context, resources);
                if (updatedDirectory) {
                    // 更新代次保持只读；随包文件收编为资源包，由共享解析选择它，不改下载的资源目录。
                    java.util.Set<String> packaged = packagedArtifacts(context);
                    for (OnDemandGroup group : ON_DEMAND_GROUPS) {
                        if (packaged.containsAll(group.artifacts())
                                && !ResourcePacks.installed(context.getFilesDir(), group.pack()))
                            ResourcePacks.adopt(context.getFilesDir(), group.pack(), resources);
                    }
                }
            }
            String error = refreshHost(configuration);
            if (error != null && error.startsWith("dictionary_outdated") && bundledDirectory) {
                extractDictionary(context, resources);
                error = refreshHost(configuration);
            }
            if (error == null && bundledDirectory && ownedState
                    && packagedArtifacts(context).containsAll(WUBI_GROUP.artifacts())) {
                // 同锁代次不会触发 refresh 的重建；重新准备已有代次，合并五笔表并回放日志，也覆盖补文件后被杀的启动。
                JSONObject request = new JSONObject().put("resources", resources.getAbsolutePath())
                    .put("state_root", state.getAbsolutePath());
                if (!AppEdition.current().isFull()) request.put("edition", AppEdition.current().id());
                JSONObject prepared = new JSONObject(NativeClient.prepareHost(request.toString()));
                if (!JsonPolicy.strictTrue(prepared.opt("ok")))
                    throw new IllegalStateException("Bundled dictionary preparation failed: "
                        + PreparationFailure.describe(new IllegalStateException(
                            JsonPolicy.strictStringOrEmpty(prepared.opt("error")))));
            }
        } catch (Exception | LinkageError error) {
            // Bootstrap has no editor or session input; never use this logging for keystrokes.
            android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed", error);
        }
    }

    /** 已有配置也要接收新安装包随带的按需组；共享层允许整组缺席，不能只靠过期错误触发解包。 */
    private static void restorePackagedOnDemandGroups(Context context, File resources) throws Exception {
        java.util.Set<String> packaged = packagedArtifacts(context);
        // 持有 bootstrap.lock，上一进程被杀后留下的原子复制暂存文件已没有写者；共享目录校验不接受它们。
        ensureSafeDirectory(resources.toPath());
        File[] entries = resources.listFiles();
        for (File entry : entries == null ? new File[0] : entries) {
            for (String name : packaged) {
                if (entry.getName().matches("\\." + java.util.regex.Pattern.quote(name) + "\\.[0-9]+\\.staging")) {
                    Files.delete(entry.toPath());
                    break;
                }
            }
        }
        for (OnDemandGroup group : ON_DEMAND_GROUPS) {
            if (!packaged.containsAll(group.artifacts())) continue;
            boolean complete = true;
            boolean present = false;
            for (String name : group.artifacts()) {
                java.nio.file.Path file = new File(resources, name).toPath();
                present |= Files.exists(file, LinkOption.NOFOLLOW_LINKS);
                complete &= Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS) && SafePaths.isSingleLink(file);
            }
            if (complete || (!present && ResourcePacks.installed(context.getFilesDir(), group.pack()))) continue;
            ensureSafeDirectory(resources.toPath());
            for (String name : group.artifacts()) {
                try (InputStream input = context.getAssets().open("dictionary/" + name)) {
                    copyAsset(input, new File(resources, name).toPath());
                }
            }
        }
    }

    /** 刷新一次配置；成功时返回 `null`，失败时记日志并返回共享层的错误文本。 */
    private static String refreshHost(File configuration) throws Exception {
        JSONObject result = new JSONObject(NativeClient.refreshHost(configuration.getAbsolutePath()));
        if (JsonPolicy.strictTrue(result.opt("ok"))) return null;
        String error = JsonPolicy.strictStringOrEmpty(result.opt("error"));
        android.util.Log.w("MSIMEBootstrap", "Runtime options refresh failed: "
            + (error.startsWith("dictionary_outdated") ? "dictionary_outdated" : "error"));
        return error;
    }

    /** 已有配置；超过 1 MiB 时为 `null`。按块读并限长，文件在检查之后变大也不会无界分配。 */
    private static JSONObject readConfiguredOptions(File configuration) throws Exception {
        if (!privateConfiguration(configuration.toPath()))
            throw new java.io.IOException("Runtime options are not a private regular file");
        try (InputStream input = Files.newInputStream(configuration.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, 1024 * 1024);
            if (bytes == null) return null;
            return new JSONObject(new String(bytes, StandardCharsets.UTF_8));
        }
    }

    static String readMarker(java.nio.file.Path file) {
        if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)
                || !SafePaths.isSingleLink(file)) return null;
        try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, 64);
            return bytes == null ? null : new String(bytes, StandardCharsets.UTF_8);
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
