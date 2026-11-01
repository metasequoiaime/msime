package app.msime.android;

import java.nio.charset.StandardCharsets;
import org.json.JSONException;
import org.json.JSONObject;

/** JNI transport for an Android IME host. Session operations use the creating thread.
 * JSON response ownership is handled inside JNI. The host parses the envelope,
 * maps handled/commit/view to InputConnection and UI, and destroys its session.
 */
public final class NativeClient {
    private static final int EMOJI_QUERY_LIMIT = 16_384;
    private static final int EMOJI_RESOURCES_LIMIT = 4_096;
    private static final int EMOJI_RESPONSE_LIMIT = 1_048_576;
    private static final int GLOSS_REQUEST_LIMIT = 262_144;
    private static final int GLOSS_RESOURCES_LIMIT = 4_096;
    private static final int GLOSS_RESPONSE_LIMIT = 1_048_576;
    /** The shared provider FFI rejects an online query document larger than this. */
    private static final int ONLINE_QUERY_LIMIT = 16_384;
    private static final int ENGLISH_COMPLETION_RESPONSE_LIMIT = 262_144;
    private static final int ENGLISH_COMPLETION_RESOURCES_LIMIT = 4 * 1024;
    /** 候选栏的合并展示上限：原文加至多十一个前缀补全。 */
    private static final int ENGLISH_SUGGESTION_DISPLAY_LIMIT = 12;
    private static final int SHUANGPIN_PROFILE_LIMIT = 64;
    private static final int SHUANGPIN_HINT_RESPONSE_LIMIT = 65_536;
    /** 与 `client_jni.cpp` 的 `kSmallJsonRequestLimit` 一致。 */
    private static final int NINE_KEY_SHUANGPIN_REQUEST_LIMIT = 16 * 1024;
    private static final int SMART_PUNCTUATION_REQUEST_LIMIT = 4_096;
    private static final int GLIDE_REQUEST_LIMIT = 65_536;
    /** 与 `msime_client_set_key_geometry` 的上限一致。 */
    private static final int KEY_GEOMETRY_LIMIT = 16_384;
    /** 与 JNI 和 C 入口一致，为 8 MiB 原始词表的 JSON 转义及元数据留出空间。 */
    private static final int VOCABULARY_REQUEST_LIMIT = 17 * 1024 * 1024;
    /** 与 C 入口 {@code msime_client_personal_wordbook_add} 的 64 KiB 上限一致。 */
    private static final int PERSONAL_WORDBOOK_REQUEST_LIMIT = 65536;
    static { System.loadLibrary("msime_android"); }
    private NativeClient() {}
    public static String create(String options) { return TextPolicy.utf8(createRaw(options.getBytes(StandardCharsets.UTF_8))); }
    public static String prepareHost(String options) { return TextPolicy.utf8(prepareHostRaw(options.getBytes(StandardCharsets.UTF_8))); }
    /** Runs the shared `msime_client_refresh_host` on the runtime options file at the absolute `path`. */
    public static String refreshHost(String path) { return TextPolicy.utf8(refreshHostRaw(path.getBytes(StandardCharsets.UTF_8))); }
    /** Reads the current native dictionary version without creating a session. */
    public static String snapshotVersion(String options) {
        return TextPolicy.utf8(snapshotVersionRaw(options.getBytes(StandardCharsets.UTF_8)));
    }
    /** May block on the shared file lock. Call on a worker, without a session handle. */
    public static String loadPreferences(String directory) { return TextPolicy.utf8(loadPreferencesRaw(directory.getBytes(StandardCharsets.UTF_8))); }
    /** 旧布局迁移在共享锁内检查原始字段，不把未知样式误当默认值。 */
    public static String migrateKeyLayout(String request) { return TextPolicy.utf8(migrateKeyLayoutRaw(request.getBytes(StandardCharsets.UTF_8))); }
    /** The global theme picker entries: ids, titles, appearance and built-in palettes, in picker order. */
    public static String themeCatalog() { return TextPolicy.utf8(themeCatalogRaw()); }
    /** 解析当前主题的单种明暗配色；带 `skins_directory` 的请求会校验包资源，必须在工作线程调用。 */
    public static String resolveTheme(String request) { return TextPolicy.utf8(resolveThemeRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String skinPackage(String request) { return TextPolicy.utf8(skinPackageRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String skinResource(String request) { return TextPolicy.utf8(skinResourceRaw(request.getBytes(StandardCharsets.UTF_8))); }
    /** 应用主题目录：`{app_themes:[{id,title,season,seasonal,light,dark}],default}`。纯计算。 */
    public static String appThemeCatalog() { return TextPolicy.utf8(appThemeCatalogRaw()); }
    /** 应用主题在本地月份与明暗模式下的颜色：`{id,season,accent,accent_soft,on_accent,background,card,hair}`。纯计算，可在主线程调用。 */
    public static String resolveAppTheme(String appTheme, int month, boolean dark) {
        validateMonth(month);
        try {
            JSONObject request = new JSONObject().put("app_theme", appTheme).put("month", month)
                .put("dark", dark);
            return TextPolicy.utf8(resolveAppThemeRaw(request.toString().getBytes(StandardCharsets.UTF_8)));
        } catch (org.json.JSONException error) {
            throw new IllegalArgumentException("Invalid app theme request", error);
        }
    }
    /** 「重置所有设置」：把偏好按修订号比较并交换换回默认值，返回新的快照。读写文件，在工作线程调用。 */
    public static String restoreDefaultPreferences(String directory, long expectedRevision) {
        if (expectedRevision < 0) throw new IllegalArgumentException("Invalid preferences revision");
        return TextPolicy.utf8(restoreDefaultPreferencesRaw(utf8(directory), expectedRevision));
    }
    /** 本版本的默认偏好文档（Android 新装默认值）。纯计算。 */
    public static String defaultPreferences() { return TextPolicy.utf8(defaultPreferencesRaw()); }
    /** 触控板设置范围和默认值。纯计算，不重复定义 Java 侧规则。 */
    public static String textTrackpadOptions() { return TextPolicy.utf8(textTrackpadOptionsRaw()); }
    /** 指定宿主（如 "android"）的能力描述。纯计算。 */
    public static String hostCapabilities(String platform) { return TextPolicy.utf8(hostCapabilitiesRaw(utf8(platform))); }
    /** 词库管理请求 `{options,action}`（列表、搜索、编辑、导入、导出）。会等待词库锁，在工作线程调用。 */
    public static String dictionary(String request) { return TextPolicy.utf8(dictionaryRaw(utf8(request))); }
    /** 个人词库队列请求，键盘活着时也能写词条和导入。在工作线程调用。 */
    public static String personalDictionaryRequest(String request) {
        return TextPolicy.utf8(personalDictionaryRequestRaw(utf8(request)));
    }
    /** 每行一个纯汉字词，答复为按规范拼音生成的词条 `{entries}`。只读内置词库，在工作线程调用。 */
    public static String dictionaryHansEntries(String text, String resources) {
        return TextPolicy.utf8(dictionaryHansEntriesRaw(utf8(text), utf8(resources)));
    }
    /** 导入前预览：词库文件 `{kind,format,text}` 解析成可入队的词条与导入报告。在工作线程调用。 */
    public static String dictionaryImportEntries(String request, String resources) {
        return TextPolicy.utf8(dictionaryImportEntriesRaw(utf8(request), utf8(resources)));
    }
    /** 内置词库的版本信息 `{profile,sourceCommit}`。读文件，在工作线程调用。 */
    public static String dictionaryManifest(String resources) { return TextPolicy.utf8(dictionaryManifestRaw(utf8(resources))); }
    /** 插件包仓库与 @ 名单请求 `{state_root,sound_packs,action}`。读写文件，在工作线程调用。 */
    public static String plugins(String request) { return TextPolicy.utf8(pluginsRaw(utf8(request))); }
    /** 社区资源本地库（保留的回复模板等）请求。持有文件锁，在工作线程调用。 */
    public static String communityResourceLibrary(String request) {
        return TextPolicy.utf8(communityResourceLibraryRaw(utf8(request)));
    }
    /** 皮肤试用的结束与崩溃恢复。写偏好，在工作线程调用。 */
    public static String keyboardSkinTrial(String request) { return TextPolicy.utf8(keyboardSkinTrialRaw(utf8(request))); }
    /** 社区皮肤安装并开始试用。写偏好与两个加锁文件，在工作线程调用。 */
    public static String communitySkinInstall(String request) { return TextPolicy.utf8(communitySkinInstallRaw(utf8(request))); }
    /** 一个按键音包校验后的文件清单 `{state_root,sound_packs,pack}`。读文件，不在按键路径上调用。 */
    public static String keySoundPack(String request) { return TextPolicy.utf8(keySoundPackRaw(utf8(request))); }
    /** 离线世界时钟 `{directory,action}`；加载、修改和快照统一在工作线程调用。 */
    public static String worldClock(String request) { return TextPolicy.utf8(worldClockRaw(utf8(request))); }
    /** 无编码常用语 `{directory,action}`，返回整份文档；设置进程和键盘进程都用。持有文件锁，在工作线程调用。 */
    public static String commonPhrases(String request) { return TextPolicy.utf8(commonPhrasesRaw(utf8(request))); }
    /** 命名词库 `{options,action}`，词条经个人词库队列送进 Engine。读写文件，在工作线程调用。 */
    public static String dictionaryCollections(String request) {
        return TextPolicy.utf8(dictionaryCollectionsRaw(utf8(request)));
    }
    /** 诊断包 `{state_root,include,sources,destination}`：写 zip，或不带 destination 时返回上传用的 sections。输入事件只保留白名单字段，配置快照已脱敏。在工作线程调用。 */
    public static String diagnosticBundle(String request) { return TextPolicy.utf8(diagnosticBundleRaw(utf8(request))); }
    /** 本机设置导出成账号设置文档的键值（可附带合并后的整份文档）；凭据与设备本地设置不导出。读偏好，在工作线程调用。 */
    public static String accountSettingsExport(String request) {
        return TextPolicy.utf8(accountSettingsExportRaw(utf8(request)));
    }
    /** 把云端设置文档应用到本机偏好并按修订号保存，返回保存后的快照、按键反馈、皮肤库与跳过的键。在工作线程调用。 */
    public static String accountSettingsApply(String request) {
        return TextPolicy.utf8(accountSettingsApplyRaw(utf8(request)));
    }

    private static void validateMonth(int month) {
        if (month < 1 || month > 12) throw new IllegalArgumentException("Month must be between 1 and 12");
    }

    /** Classifies committed text and adds batched per-key press counts in native memory, and persists only aggregate counts. Call on a worker. */
    public static String typingStatistics(String request) {
        return TextPolicy.utf8(typingStatisticsRaw(request.getBytes(StandardCharsets.UTF_8)));
    }
    /**
     * Whether aggregate statistics are switched on in the store under {@code directory}. A missing or unreadable document reads as off, so a capture gate built on this never records by default. Takes the shared file lock: call on a worker, on activation, never per key.
     */
    public static boolean typingStatisticsEnabled(String directory) {
        return typingStatisticsEnabledRaw(directory.getBytes(StandardCharsets.UTF_8)) == 1;
    }
    /**
     * 背单词: one review action, answered with the whole status. Takes the shared file lock and may
     * read a multi-megabyte wordbook, so call on a worker and never from the input path.
     *
     * <p>The limit is larger than every other request here because this one can carry an imported
     * word list: a five-thousand-word CET book is a few hundred kilobytes of text. The shared
     * parser applies its own row and entry ceilings underneath.
     */
    public static String vocabularyReview(String request) {
        byte[] bytes = request.getBytes(StandardCharsets.UTF_8);
        if (!HttpBodyPolicy.isNonEmptyWithinLimit(bytes, VOCABULARY_REQUEST_LIMIT)) {
            return "{\"ok\":false,\"error\":\"invalid vocabulary review request\"}";
        }
        return TextPolicy.utf8(vocabularyReviewRaw(bytes));
    }
    /**
     * 长按候选菜单的「加入生词本」：把候选原文、显示着的释义和读音交给共享层（C ABI {@code msime_client_personal_wordbook_add}），返回它答的那句给用户看的提示；请求不合法或存储出错时返回 null。取词书目录的文件锁、读写生词本，在工作线程调用。
     */
    public static String personalWordbookAdd(String directory, String candidate, String gloss, String pronunciation) {
        byte[] bytes;
        try {
            bytes = new JSONObject().put("directory", directory).put("candidate", candidate)
                .put("gloss", gloss).put("pronunciation", pronunciation)
                .toString().getBytes(StandardCharsets.UTF_8);
        } catch (JSONException error) {
            return null;
        }
        if (!HttpBodyPolicy.isNonEmptyWithinLimit(bytes, PERSONAL_WORDBOOK_REQUEST_LIMIT)) return null;
        try {
            JSONObject envelope = new JSONObject(TextPolicy.utf8(personalWordbookAddRaw(bytes)));
            if (!JsonPolicy.strictTrue(envelope.opt("ok"))) return null;
            JSONObject value = envelope.optJSONObject("value");
            String notice = value == null ? "" : JsonPolicy.strictStringOrEmpty(value.opt("notice"));
            return notice.isEmpty() ? null : notice;
        } catch (JSONException error) {
            return null;
        }
    }
    /**
     * 长按候选菜单的「朗读」和背单词卡片的喇叭要读的英文（C ABI {@code msime_client_read_aloud_text}）：取每行释义的第一个义项，第一个是英文的就是答案；没有时候选本身是英文就读候选。没有能读的、请求不合法时返回 null，菜单就不出这一项。不读写文件，可以在主线程打开菜单时调用。背单词卡片把词头当作 {@code candidate}、{@code gloss} 给空串。
     */
    public static String readAloudText(String candidate, String gloss) {
        byte[] bytes;
        try {
            bytes = new JSONObject().put("candidate", candidate == null ? "" : candidate)
                .put("gloss", gloss == null ? "" : gloss).toString().getBytes(StandardCharsets.UTF_8);
        } catch (JSONException error) {
            return null;
        }
        if (!HttpBodyPolicy.isNonEmptyWithinLimit(bytes, PERSONAL_WORDBOOK_REQUEST_LIMIT)) return null;
        try {
            JSONObject envelope = new JSONObject(TextPolicy.utf8(readAloudTextRaw(bytes)));
            if (!JsonPolicy.strictTrue(envelope.opt("ok"))) return null;
            String text = JsonPolicy.strictStringOrEmpty(envelope.opt("value"));
            return text.isEmpty() ? null : text;
        } catch (JSONException error) {
            return null;
        }
    }
    /** Reads one bounded page from the verified packaged emoji catalog. Call on a worker. */
    public static String emojiCatalog(String query, String resources) {
        byte[] queryBytes = query.getBytes(StandardCharsets.UTF_8);
        byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
        if (queryBytes.length > EMOJI_QUERY_LIMIT || resourcesBytes.length > EMOJI_RESOURCES_LIMIT)
            throw new IllegalArgumentException("Emoji catalog request is too large");
        byte[] result = emojiCatalogRaw(queryBytes, resourcesBytes);
        if (!HttpBodyPolicy.isWithinLimit(result, EMOJI_RESPONSE_LIMIT))
            throw new IllegalStateException("Emoji catalog response is too large");
        return TextPolicy.utf8(result);
    }
    /** Resolves copied candidates against the packaged offline dictionary. Call on a worker. */
    public static String candidateGlosses(String request, String resources) {
        byte[] requestBytes = request.getBytes(StandardCharsets.UTF_8);
        byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
        if (requestBytes.length > GLOSS_REQUEST_LIMIT
                || resourcesBytes.length > GLOSS_RESOURCES_LIMIT)
            throw new IllegalArgumentException("Candidate gloss request is too large");
        byte[] result = candidateGlossesRaw(requestBytes, resourcesBytes);
        if (!HttpBodyPolicy.isWithinLimit(result, GLOSS_RESPONSE_LIMIT))
            throw new IllegalStateException("Candidate gloss response is too large");
        return TextPolicy.utf8(result);
    }
    /** 从同级的固定离线音标表读取读音，只在工作线程调用。 */
    public static String candidatePronunciations(String request, String resources,
            String supplementaryResources) {
        if (supplementaryResources != null && !supplementaryResources.isEmpty()) {
            try {
                request = new JSONObject(request).put("supplementary_resources", supplementaryResources).toString();
            } catch (JSONException error) {
                throw new IllegalArgumentException("Invalid pronunciation request", error);
            }
        }
        byte[] requestBytes = request.getBytes(StandardCharsets.UTF_8);
        byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
        if (requestBytes.length > GLOSS_REQUEST_LIMIT || resourcesBytes.length > GLOSS_RESOURCES_LIMIT)
            throw new IllegalArgumentException("Candidate pronunciation request is too large");
        byte[] result = candidatePronunciationsRaw(requestBytes, resourcesBytes);
        if (!HttpBodyPolicy.isWithinLimit(result, GLOSS_RESPONSE_LIMIT))
            throw new IllegalStateException("Candidate pronunciation response is too large");
        return TextPolicy.utf8(result);
    }
    /**
     * 同 {@link #candidateGlosses(String, String)}，另把 {@code stateRoot}（{@code filesDir/bootstrap/state}）写进请求的 {@code state_root}：资源目录旁没有离线释义时，非英语的释义改从下载的 offline-glosses 资源包里读。{@code stateRoot} 为 null 时与原方法相同。Call on a worker.
     */
    public static String candidateGlosses(String request, String resources, String stateRoot) {
        if (stateRoot == null) return candidateGlosses(request, resources);
        try {
            return candidateGlosses(new JSONObject(request).put("state_root", stateRoot).toString(),
                resources);
        } catch (org.json.JSONException error) {
            throw new IllegalArgumentException("Invalid candidate gloss request", error);
        }
    }
    /** 在工作线程读取候选栏的合并展示：有纠错时为原文与纠错，原文已是完整的词时只有原文，否则原文后接前缀补全。 */
    public static String englishSuggestions(String prefix, String resources) {
        return englishCandidates(prefix, resources, true);
    }

    /** 共享引擎返回原文第一行及可选的唯一纠错，不改变输入会话；空格和标点边界只用这一查询。 */
    public static String englishCorrections(String prefix, String resources) {
        return englishCandidates(prefix, resources, false);
    }

    private static String englishCandidates(String prefix, String resources, boolean completions) {
        if (prefix == null || prefix.isEmpty() || prefix.length() > 128
                || !prefix.chars().allMatch(value -> TextPolicy.isAsciiLetter(value) || value == '\''))
            throw new IllegalArgumentException("Invalid English completion prefix");
        try {
            JSONObject request = new JSONObject().put("prefix", prefix)
                .put("limit", completions ? ENGLISH_SUGGESTION_DISPLAY_LIMIT : 2).put("correction", true);
            if (completions) request.put("completions", true);
            byte[] requestBytes = request.toString().getBytes(StandardCharsets.UTF_8);
            byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
            if (resourcesBytes.length > ENGLISH_COMPLETION_RESOURCES_LIMIT)
                throw new IllegalArgumentException("English completion resources are too large");
            byte[] result = englishCompletionsRaw(requestBytes, resourcesBytes);
            if (!HttpBodyPolicy.isWithinLimit(result, ENGLISH_COMPLETION_RESPONSE_LIMIT))
                throw new IllegalStateException("English completion response is too large");
            return TextPolicy.utf8(result);
        } catch (org.json.JSONException error) {
            throw new IllegalArgumentException("Invalid English completion request", error);
        }
    }
    /**
     * The polish prompt the selected slot resolves to.
     *
     * <p>Decided by the shared C++ header the other hosts read, not by this host: slot precedence has been wrong on individual hosts before, and the preset bodies carry their own prompt-injection wording that must not drift between copies.
     */
    public static String polishPrompt(String id, String custom1, String custom2, String custom3) {
        return TextPolicy.utf8(polishPromptRaw(utf8(id), utf8(custom1), utf8(custom2), utf8(custom3)));
    }

    private static byte[] utf8(String value) {
        return TextPolicy.utf8Bytes(value);
    }

    /**
     * The one clipboard history every mobile host shares.
     *
     * <p>Ordering, the fifty-entry limit, pinning and eviction are the shared store's, and the file
     * is the one the settings page reads. A second implementation here would be a second history.
     */
    public static String mobileClipboardHistory(String request) {
        return TextPolicy.utf8(mobileClipboardHistoryRaw(utf8(request)));
    }

    /** 共享 `msime_client_cloud_clipboard_e2e` 的请求上限：继续一步的请求带着一整页密文响应。 */
    public static final int CLOUD_CLIPBOARD_E2E_REQUEST_LIMIT = 2 * 1024 * 1024;

    /**
     * 云剪贴板端到端加密驱动的一步：`{start,keystore,device_name}` 开始一个操作，`{resume,response}` 交回上一步请求的响应，返回 `{"ok":true,"value":…}`。
     *
     * <p>驱动不联网、不碰会话，也不落盘；请求由宿主带着账号凭据发出，密钥串由宿主保存（见 {@link CloudClipboardE2e}）。协议见 `docs/cloud-clipboard-e2e.md`。
     */
    public static String cloudClipboardE2e(String request) {
        byte[] payload = utf8(request);
        if (payload.length > CLOUD_CLIPBOARD_E2E_REQUEST_LIMIT)
            throw new IllegalArgumentException("Cloud clipboard request is too large");
        return TextPolicy.utf8(cloudClipboardE2eRaw(payload));
    }

    /** 共享 `msime_client_cloud_clipboard_source` 的偏好目录上限。 */
    public static final int CLOUD_CLIPBOARD_SOURCE_DIRECTORY_LIMIT = 16 * 1024;
    /** 共享 `msime_client_relay_cloud_clipboard` 和 `msime_client_relay_cloud_clipboard_exchange` 的请求上限。 */
    public static final int CLOUD_CLIPBOARD_RELAY_REQUEST_LIMIT = 128 * 1024;

    /**
     * 云剪贴板该走哪条路：`directory` 是共享偏好目录，返回 `{"ok":true,"value":{"source":"relay"|"account"}}`；读不了偏好时 error 是 `storage`，中继配置不合规时是 `relay_not_configured`。只读文件、不联网。
     *
     * <p>配置了中继就只走中继，见 {@link CloudClipboardRelay}。
     */
    public static String cloudClipboardSource(String directory) {
        byte[] payload = utf8(directory);
        if (payload.length > CLOUD_CLIPBOARD_SOURCE_DIRECTORY_LIMIT)
            throw new IllegalArgumentException("Preferences directory is too long");
        return TextPolicy.utf8(cloudClipboardSourceRaw(payload));
    }

    /**
     * 经自建中继答一个云剪贴板动作：请求是 `{preferences_directory, directory, device_name, action}`，格式和错误码见 `crates/host-api/include/msime_client.h` 与 `docs/cloud-clipboard-relay.md`。
     *
     * <p>Android 只用它发不涉及内容、不碰密钥串的动作（开关、置顶、保留、删除、清空）：经它跑端到端加密的动作时 Rust 会把密钥串写成明文文件，Android 的密钥串改由 {@link CloudClipboardRelay} 用 AndroidKeyStore 加密保存，那些动作走 {@link #relayCloudClipboardExchange}。
     *
     * <p>阻塞在网络上（每个请求最多 30 秒），只在工作线程上调用，不要在主线程或输入线程上调用。
     */
    public static String relayCloudClipboard(String request) {
        byte[] payload = utf8(request);
        if (payload.length > CLOUD_CLIPBOARD_RELAY_REQUEST_LIMIT)
            throw new IllegalArgumentException("Cloud clipboard relay request is too large");
        return TextPolicy.utf8(cloudClipboardRelayRaw(payload));
    }

    /**
     * 向中继发端到端加密驱动给出的一个请求：请求是 `{url, token, method, path, body}`（WebDAV 网盘是 `{backend:"webdav", url, username, password, directory, method, path, body}`），`url` 和 `token` 是这次操作开始时从偏好里读到的那一份；返回 `{"ok":true,"value":{"status":…,"body":"…"}}`，状态码和正文原样交回驱动。Rust 只负责校验地址和令牌、把请求限定在中继基地址下的云剪贴板接口、带上令牌、限制响应大小，不读偏好、不读写任何密钥串。请求里有令牌，不要写日志。错误码见 `crates/host-api/include/msime_client.h`。
     *
     * <p>阻塞在网络上（最多 30 秒），只在工作线程上调用。
     */
    public static String relayCloudClipboardExchange(String request) {
        byte[] payload = utf8(request);
        if (payload.length > CLOUD_CLIPBOARD_RELAY_REQUEST_LIMIT)
            throw new IllegalArgumentException("Cloud clipboard relay request is too large");
        return TextPolicy.utf8(cloudClipboardRelayExchangeRaw(payload));
    }

    /**
     * WebDAV 设置的「测试连接」：请求是 `{url, username, password, directory?}`（设置页里还没保存的草稿；`directory` 是中继密钥串的目录，通过后删掉那里的日志缓存），通过时返回 `{"ok":true,"value":{}}`。不合格是 `webdav_unsupported`，账号或密码不对是 `relay_unauthorized`。格式见 `crates/host-api/include/msime_client.h`。请求里有应用密码，不要写日志。
     *
     * <p>阻塞在网络上（大约二十个请求，每个最多 30 秒），只在工作线程上调用。
     */
    public static String cloudClipboardWebDavProbe(String request) {
        byte[] payload = utf8(request);
        if (payload.length > CLOUD_CLIPBOARD_RELAY_REQUEST_LIMIT)
            throw new IllegalArgumentException("Cloud clipboard WebDAV probe request is too large");
        return TextPolicy.utf8(cloudClipboardWebDavProbeRaw(payload));
    }

    /** 共享 `msime_client_dictionary_sync` 的请求上限：HostOptions 文档里带着整份偏好。 */
    public static final int DICTIONARY_SYNC_REQUEST_LIMIT = 2 * 1024 * 1024;

    /**
     * 「我的词条」经 WebDAV 网盘同步（`docs/dictionary-webdav-sync.md`）：请求是 `{options, directory, device_name, action}`，`options` 是 runtime-options.json 那份 HostOptions，`directory` 是两个进程共用的 `noBackupFilesDir/dictionary-sync`，`action` 与返回的 `{"ok":true,"value":…}` 见 `crates/host-api/include/msime_client.h`。网盘的地址、账号和应用密码由 Rust 从偏好的 `dictionary_sync_webdav` 读；「测试连接」（`probe`）的请求里带着设置页草稿里的应用密码，不要写日志。
     *
     * <p>除 `status` 外都会阻塞在网络上（每个请求最多 30 秒，整次同步持有本机的同步锁，另一个进程正在同步时最多再等两分钟），只在工作线程上调用，不要在主线程或输入线程上调用。封装见 {@link DictionaryWebDavSync}。
     */
    public static String dictionarySync(String request) {
        byte[] payload = utf8(request);
        if (payload.length > DICTIONARY_SYNC_REQUEST_LIMIT)
            throw new IllegalArgumentException("Dictionary sync request is too large");
        return TextPolicy.utf8(dictionarySyncRaw(payload));
    }

    /**
     * Decode a Doubao streaming response frame with the shared implementation.
     *
     * <p>The protocol's framing and auth modes are shared across hosts; Android supplies only the
     * transport.
     */
    public static String doubaoDecodeFrame(byte[] frame) {
        return TextPolicy.utf8(doubaoDecodeFrameRaw(frame));
    }

    /** The Doubao session's opening frame, or null when the shared builder refused the options. */
    public static byte[] doubaoStartFrame(boolean itn, boolean punctuation, boolean ddc,
                                          String boostingTableId) {
        return doubaoStartFrameRaw(itn, punctuation, ddc, utf8(boostingTableId));
    }

    /** One audio frame. `finalChunk` closes the utterance; the sequence must not repeat. */
    public static byte[] doubaoAudioFrame(int sequence, byte[] pcm, int length, boolean finalChunk) {
        if (length < 0) throw new IllegalArgumentException("Invalid PCM length");
        return doubaoAudioFrameRaw(sequence, pcm, length, finalChunk);
    }

    /** Reads one bounded double-pinyin key-hint map from the Engine profile tables. */
    public static String shuangpinKeyHints(String profile) {
        if (profile == null) throw new IllegalArgumentException("Missing double-pinyin profile");
        byte[] profileBytes = profile.getBytes(StandardCharsets.UTF_8);
        if (!HttpBodyPolicy.isNonEmptyWithinLimit(profileBytes, SHUANGPIN_PROFILE_LIMIT))
            throw new IllegalArgumentException("Double-pinyin profile is too large");
        byte[] result = shuangpinKeyHintsRaw(profileBytes);
        if (!HttpBodyPolicy.isWithinLimit(result, SHUANGPIN_HINT_RESPONSE_LIMIT))
            throw new IllegalStateException("Double-pinyin hint response is too large");
        return TextPolicy.utf8(result);
    }
    /**
     * 九键双拼方案的表、用户表的问题和每个数字键的键面（`msime_client_nine_key_shuangpin_layout`）。`request` 是 `{"profile": ..., "table"?: ...}`；不碰文件也不用会话句柄，键盘和设置页的编辑器都从这里取键位，不另存一份。
     */
    public static String nineKeyShuangpinLayout(String request) {
        if (request == null) throw new IllegalArgumentException("Missing nine-key shuangpin request");
        byte[] bytes = request.getBytes(StandardCharsets.UTF_8);
        if (bytes.length == 0 || bytes.length > NINE_KEY_SHUANGPIN_REQUEST_LIMIT)
            throw new IllegalArgumentException("Nine-key shuangpin request is too large");
        byte[] result = nineKeyShuangpinLayoutRaw(bytes);
        if (result == null || result.length > SHUANGPIN_HINT_RESPONSE_LIMIT)
            throw new IllegalStateException("Nine-key shuangpin response is too large");
        return TextPolicy.utf8(result);
    }
    /** May block on the shared file lock. Call on a worker, without a session handle. */
    public static String savePreferences(String directory, long expectedRevision, String snapshot) {
        if (expectedRevision < 0) throw new IllegalArgumentException("Invalid preferences revision");
        return TextPolicy.utf8(savePreferencesRaw(directory.getBytes(StandardCharsets.UTF_8), expectedRevision,
            snapshot.getBytes(StandardCharsets.UTF_8)));
    }
    /** Usage reporting (msime_client_telemetry_*): begin, end and clear touch only files; flush blocks on the network. Call on a worker. */
    public static String telemetryBegin(String request) { return TextPolicy.utf8(telemetryBeginRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String telemetryEnd(String request) { return TextPolicy.utf8(telemetryEndRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String telemetryFlush(String request) { return TextPolicy.utf8(telemetryFlushRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String telemetryClear(String request) { return TextPolicy.utf8(telemetryClearRaw(request.getBytes(StandardCharsets.UTF_8))); }
    /** The app notices feed (cached for a minute, dismissed ones left out). Blocks on the network: call on a worker, from the app, never from the input method. */
    public static String notices(String request) { return TextPolicy.utf8(noticesRaw(request.getBytes(StandardCharsets.UTF_8))); }
    public static String noticeDismiss(String request) { return TextPolicy.utf8(noticeDismissRaw(request.getBytes(StandardCharsets.UTF_8))); }
    /** Applies a bounded batch of queued personal dictionary edits. Call with no active session. */
    public static String personalDictionarySync(String options) {
        return TextPolicy.utf8(personalDictionarySyncRaw(options.getBytes(StandardCharsets.UTF_8)));
    }
    public static String focus(long session, boolean focused) { return TextPolicy.utf8(focusRaw(session, focused)); }
    /** 标出隐私会话：选词位置和上屏效率不记入打字统计。 */
    public static String setPrivateSession(long session, boolean enabled) {
        return TextPolicy.utf8(setPrivateSessionRaw(session, enabled));
    }

    public static String setNineKeyMode(long session, boolean enabled) {
        return TextPolicy.utf8(setNineKeyModeRaw(session, enabled));
    }
    /** 换引擎的组码网格；只在组字结束后调用，`View.key_grid` 是权威状态。 */
    public static String setKeyGrid(long session, int grid) {
        if (grid != KEY_GRID_NONE && grid != KEY_GRID_NINE_KEY && grid != KEY_GRID_FOURTEEN_KEY
                && grid != KEY_GRID_EIGHTEEN_KEY)
            throw new IllegalArgumentException("Unknown key grid");
        return TextPolicy.utf8(setKeyGridRaw(session, grid));
    }
    /** `msime_client_set_key_grid` 的取值：关掉网格。 */
    public static final int KEY_GRID_NONE = 0;
    /** 九键（同 {@link #setNineKeyMode}）。 */
    public static final int KEY_GRID_NINE_KEY = 1;
    /** 当前全拼或双拼方案的 14 键。 */
    public static final int KEY_GRID_FOURTEEN_KEY = 2;
    /** 全拼十八键，与共享 C ABI 布局编号一致。 */
    public static final int KEY_GRID_EIGHTEEN_KEY = 4;
    /** 字母组网格的一键：小写送整组；仅十八键双拼的大写 ASCII 表示精确小写字母。返回与 {@link #character} 相同的输入响应。 */
    public static String gridKey(long session, char letter) {
        if ((letter < 'a' || letter > 'z') && (letter < 'A' || letter > 'Z'))
            throw new IllegalArgumentException("Grid key must be an ASCII letter");
        return TextPolicy.utf8(gridKeyRaw(session, letter));
    }
    public static String setEnglishMode(long session, boolean enabled) {
        return TextPolicy.utf8(setEnglishModeRaw(session, enabled));
    }
    /**
     * The transcription provider and optional rewrite this device is configured for.
     *
     * <p>The same resolution the settings app performs, read from the same document. The keyboard
     * has its own voice entry and never goes through that app, so without this it would either
     * ignore the configuration or grow a second copy of the rules.
     */
    public static String mobileVoiceConfiguration(String directory) {
        return TextPolicy.utf8(mobileVoiceConfigurationRaw(utf8(directory)));
    }

    /** 识别服务设置行的共享状态文案；纯计算，不读写偏好。 */
    public static String mobileVoiceStatus(String provider, boolean resolved,
            boolean platformAvailable, boolean modelReady, boolean runtimeReady) {
        if (provider == null || !TextPolicy.validUnicode(provider) || provider.length() > 256
                || TextPolicy.utf8Length(provider) > 256)
            throw new IllegalArgumentException("Invalid voice status provider");
        return TextPolicy.utf8(mobileVoiceStatusRaw(utf8(provider), resolved, platformAvailable,
            modelReady, runtimeReady));
    }

    /** 语音润色设置行的共享状态文案；纯计算，不读写偏好。 */
    public static String mobileVoicePolishStatus(boolean enabled, boolean configured,
            boolean hasToken) {
        return TextPolicy.utf8(mobileVoicePolishStatusRaw(enabled, configured, hasToken));
    }

    /** 共享语音设置的服务分类和豆包鉴权模式；纯计算，不读写偏好。 */
    public static String mobileVoicePolicy(String provider, String authMode) {
        if (provider == null || authMode == null || !TextPolicy.validUnicode(provider)
                || !TextPolicy.validUnicode(authMode) || provider.length() > 256
                || authMode.length() > 256 || TextPolicy.utf8Length(provider) > 256
                || TextPolicy.utf8Length(authMode) > 256)
            throw new IllegalArgumentException("Invalid voice provider policy");
        return TextPolicy.utf8(mobileVoicePolicyRaw(utf8(provider), utf8(authMode)));
    }

    /** 共享语音服务与豆包流式接口目录；纯计算，不读写偏好、不联网。 */
    public static String voiceProviderCatalog() { return TextPolicy.utf8(voiceProviderCatalogRaw()); }
    public static String aiProviderCatalog() { return TextPolicy.utf8(aiProviderCatalogRaw()); }
    public static String aiProviderUpdate(String request) { return TextPolicy.utf8(aiProviderUpdateRaw(utf8(request))); }
    public static String aiModels(String request) { return TextPolicy.utf8(aiModelsRaw(utf8(request))); }

    /** 设置页的语音接口预检；纯计算，与 C ABI 共用 64 KiB 的 UTF-8 传输上限。 */
    public static String voiceEndpointIssue(String endpoint, boolean websocket) {
        if (endpoint == null || endpoint.length() > 65_536 || !TextPolicy.validUnicode(endpoint))
            throw new IllegalArgumentException("Invalid voice endpoint");
        byte[] payload = utf8(endpoint);
        if (payload.length > 65_536) throw new IllegalArgumentException("Invalid voice endpoint");
        return TextPolicy.utf8(voiceEndpointIssueRaw(payload, websocket));
    }

    /** 设置页的共享语音凭据预检；空值表示删除，响应不含凭据。 */
    public static String voiceCredentialIssue(String provider, String value) {
        if (provider == null || value == null || !TextPolicy.validUnicode(provider)
                || !TextPolicy.validUnicode(value) || provider.length() > 256
                || TextPolicy.utf8Length(provider) > 256 || value.length() > 16 * 1024
                || TextPolicy.utf8Length(value) > 16 * 1024)
            throw new IllegalArgumentException("Invalid voice credential");
        return TextPolicy.utf8(voiceCredentialIssueRaw(utf8(provider), utf8(value)));
    }

    /** 生成流式 WebSocket 的共享传输描述；失败时返回标准错误信封，不回退本地 URI 解析。 */
    public static String voiceWebSocketEndpoint(String endpoint) {
        if (endpoint == null || endpoint.length() > 65_536 || !TextPolicy.validUnicode(endpoint))
            throw new IllegalArgumentException("Invalid voice websocket endpoint");
        byte[] payload = utf8(endpoint);
        if (payload.length > 65_536) throw new IllegalArgumentException("Invalid voice websocket endpoint");
        return TextPolicy.utf8(voiceWebSocketEndpointRaw(payload));
    }

    /** 纯计算的换服务补丁，含凭据；不得记录请求或响应，与 C ABI 共用 1 MiB 的 UTF-8 上限。 */
    public static String voiceProviderUpdate(String request) {
        if (request == null || request.length() > 1_048_576)
            throw new IllegalArgumentException("Voice provider request is too large");
        byte[] payload = utf8(request);
        if (payload.length > 1_048_576)
            throw new IllegalArgumentException("Voice provider request is too large");
        return TextPolicy.utf8(voiceProviderUpdateRaw(payload));
    }

    /**
     * The user's own pinyin dictionary words as recognition hotwords: `{"options": HostOptions, "limit": n}` in, `{"hotwords":[{"text","pinyin"}]}` out. Reads the dictionary store, so call on a worker.
     */
    public static String voiceHotwords(String request) {
        return TextPolicy.utf8(voiceHotwordsRaw(utf8(request)));
    }

    /** Pinyin-similarity hotword replacement over a final transcript: `{"text","hotwords"}` in, `{"text"}` out. Pure. */
    public static String voiceHotwordCorrect(String request) {
        return TextPolicy.utf8(voiceHotwordCorrectRaw(utf8(request)));
    }

    /** 本地模型最终文本里汉字写的数字改成数字：`{"text","kind"}` in, `{"text"}` out。只改写不自带 ITN 的 kind，其他 kind 原样返回；只用于最终文本。Pure. */
    public static String voiceNormalizeNumbers(String request) {
        return TextPolicy.utf8(voiceNormalizeNumbersRaw(utf8(request)));
    }

    /** Whether the sherpa-onnx runtime loads: the one packaged in the APK, or the one named by {@link #localSpeechRuntime}. Loads it on the first call, so call on a worker. */
    public static boolean localSpeechAvailable() {
        return localSpeechAvailableRaw();
    }

    /**
     * 指定 libsherpa-onnx-c-api.so 的绝对路径（下载的 voice-runtime 资源包里那份）。它依赖的 libonnxruntime.so 要先用 {@link System#load} 按绝对路径载入。
     *
     * <p>之前没加载成功的结果作废，下一次 {@link #localSpeechAvailable} 按这个路径重试；运行库已经加载后调用不起作用。
     */
    public static void localSpeechRuntime(String sherpaLibrary) {
        localSpeechRuntimeRaw(utf8(sherpaLibrary));
    }

    /** 资源包安装的进度。在调用 {@link #resourcePackInstall} 的工作线程上调用；phase 是 "download"（从本地文件安装时是 "import"）、"verify" 或 "done"。不要在这里抛异常或做耗时的事。 */
    public interface ResourcePackProgress {
        void onProgress(String phase, long done, long total);
    }

    /** 每个按需资源包的安装状态：请求 {@code {"state_root"}}，返回 {@code {ok, value:[{id, state, size, schemes}]}}。只读几个文件属性。 */
    public static String resourcePacks(String requestJson) {
        return TextPolicy.utf8(resourcePacksRaw(utf8(requestJson)));
    }

    /**
     * 下载、校验并发布一个资源包：请求 {@code {"state_root","pack","sources"?:[...]}}，返回 {@code {ok, value:{path}}} 或 {@code {ok:false, error}}。
     *
     * <p>阻塞到结束，只在主进程的工作线程上调用，不在 UI 线程，也不在 :ime 进程。listener 可以为 null。
     */
    public static String resourcePackInstall(String requestJson, ResourcePackProgress listener) {
        return TextPolicy.utf8(resourcePackInstallRaw(utf8(requestJson), listener));
    }

    /** 让正在安装的资源包尽快停下，安装调用随后以 local_model_cancelled 失败；pack 为 null 时停下本进程里所有资源包的安装。任何线程，立即返回。 */
    public static void resourcePackCancel(String pack) {
        resourcePackCancelRaw(pack == null ? null : pack.getBytes(StandardCharsets.UTF_8));
    }

    /** 卸载已发布的资源包：请求 {@code {"state_root","pack"}}，返回 {@code {ok,value:{removed}}}。工作线程调用。 */
    public static String resourcePackRemove(String requestJson) {
        return TextPolicy.utf8(resourcePackRemoveRaw(utf8(requestJson)));
    }

    /**
     * 注入给 Rust 的会话存储（{@code msime_client_session_store}）的 Java 一侧：只提供字节，编解码、布局、校验和刷新都在 Rust。只在主进程里注册一次，同一份存储注册两次会各自缓存会话、拿同一个会轮换的刷新令牌各刷新一次。
     *
     * <p>Rust 可能在任意线程上并发调用这三个方法，包括 Rust 自己起的线程；方法里不能再调 NativeClient 的账号与云端入口，也不能等待账号刷新。抛出的异常按失败处理，在 JNI 一侧清掉，不会抛给调用 Rust 的那一方。
     */
    public interface SessionStore {
        /** 登录会话。 */
        int ITEM_ACCOUNT = 1;
        /** 云剪贴板密钥串，属于登录会话，清掉登录会话时一起删除。 */
        int ITEM_CLIPBOARD_KEYSTORE = 2;
        /** 本机匿名账号的会话。 */
        int ITEM_ANONYMOUS = 3;
        /** 匿名账号的身份，save 是「不存在才写」。 */
        int ITEM_ANONYMOUS_IDENTITY = 4;

        /** 该项最后一次成功写入的字节；不存在时返回 null，空数组也按不存在处理。比 Rust 给的上限（会话和身份 65536 字节，密钥串 4096 字节）长的按失败处理。 */
        byte[] load(int item);

        /** 原子地整体替换该项，返回 0 是已写入。{@link #ITEM_ANONYMOUS_IDENTITY} 已经有值时不覆盖、返回 1。其他返回值是失败。 */
        int save(int item, byte[] bytes);

        /** 删掉该项，不存在也算成功：返回 0 是成功，其他值是失败。 */
        int remove(int item);
    }

    /** 流式对话的一段增量（UTF-8，不会把一个字符拆在两段里），在调用流式对话的那个工作线程上按到达顺序收到。可以在这里取消那次对话；抛出异常以后不再收到增量，对话照常进行到结束，然后异常抛给调用方。 */
    public interface ChatDelta {
        void onDelta(byte[] text);
    }

    /** AI 皮肤每画完一张插画收到一次，completed 从 1 到 3，在调用生成的那个工作线程上。可以在这里取消那次生成；抛出异常时的处理与 {@link ChatDelta} 相同。 */
    public interface AiSkinProgress {
        void onProgress(int completed);
    }

    /** 注册会话存储（{@code msime_client_session_store_register}），只给 {@link AccountStore} 用：返回信封，成功时 value 是 {@code {session_store}}。clientAgent 为 null 时请求以 MSIME/Android 发出。不联网。 */
    static String sessionStoreRegister(SessionStore store, String clientAgent) {
        return TextPolicy.utf8(sessionStoreRegisterRaw(store, clientAgent == null ? null : utf8(clientAgent)));
    }

    /** 由 Rust 拼出本宿主自报的 User-Agent（{@code msime_client_client_agent}）。不联网，任意线程。 */
    static String clientAgent(String request) { return TextPolicy.utf8(clientAgentRaw(utf8(request))); }

    /** 昵称的共享纯计算；严格 UTF-8，传输上限与 C 接口一致。 */
    public static String accountName(String request) {
        if (request == null || request.length() > 16 * 1024 || !TextPolicy.validUnicode(request))
            throw new IllegalArgumentException("Invalid account name request");
        byte[] payload = utf8(request);
        if (payload.length > 16 * 1024) throw new IllegalArgumentException("Invalid account name request");
        return TextPolicy.utf8(accountNameRaw(payload));
    }

    /** 账号操作（{@code msime_client_account}），请求带 {@code session_store}。除 status 和 forget 外都阻塞网络，经存储的调用都不能放在主线程上。 */
    static String account(String request) { return TextPolicy.utf8(accountRaw(utf8(request))); }

    /** 登录的优先、否则匿名账号的访问令牌（{@code msime_client_account_access_token}）。可能刷新令牌、阻塞网络；回复含凭据，不要写进日志。 */
    static String accountAccessToken(String request) { return TextPolicy.utf8(accountAccessTokenRaw(utf8(request))); }

    /** 账号释义的 sans-IO 一半（{@code msime_client_account_gloss}），只给 {@link BackendTranslationClient} 用：拼请求体、解读收到的响应，不联网、不读会话，两个进程都能调，任意线程。 */
    static String accountGloss(String request) { return TextPolicy.utf8(accountGlossRaw(utf8(request))); }

    /** 社区操作（{@code msime_client_community}），请求带 {@code session_store}。阻塞网络，放在工作线程上。 */
    static String community(String request) { return TextPolicy.utf8(communityRaw(utf8(request))); }

    /** 云剪贴板的账号路径（{@code msime_client_account_cloud_clipboard}），请求带 {@code session_store}。阻塞网络，放在工作线程上。 */
    static String accountCloudClipboard(String request) { return TextPolicy.utf8(accountCloudClipboardRaw(utf8(request))); }

    /** 流式对话（{@code msime_client_account_chat_stream}）：增量在调用线程上交给 listener，最长阻塞 125 秒，放在工作线程上。 */
    static String accountChatStream(String request, ChatDelta listener) {
        return TextPolicy.utf8(accountChatStreamRaw(utf8(request), listener));
    }

    /** 取消一次流式对话（{@code {request_id}}），任意线程，立即返回。 */
    static String accountChatCancel(String request) { return TextPolicy.utf8(accountChatCancelRaw(utf8(request))); }

    /** AI 皮肤一次生成三套（{@code msime_client_ai_skin_generate}）：最长约 5 分钟，放在工作线程上。 */
    static String aiSkinGenerate(String request, AiSkinProgress listener) {
        return TextPolicy.utf8(aiSkinGenerateRaw(utf8(request), listener));
    }

    /** 取消一次 AI 皮肤生成（{@code {request_id}}），任意线程，立即返回。 */
    static String aiSkinCancel(String request) { return TextPolicy.utf8(aiSkinCancelRaw(utf8(request))); }

    /** 词库快照激活队列（{@code msime_client_snapshot_queue}），只给 {@link DictionarySnapshotQueue} 用。不读会话，主进程和 `:ime` 都能调；会读写文件，process 还会准备和激活整份快照，放在工作线程上。 */
    static String snapshotQueue(String request) { return TextPolicy.utf8(snapshotQueueRaw(utf8(request))); }

    /** 自动云同步（{@code msime_client_cloud_sync}），只给 {@link SyncSignals} 和 {@link RustCloudSync} 用。peek、mark_dirty 和 import_legacy 不读会话，两个进程都能调；其余操作带 {@code session_store}，只在主进程里调。run 会联网，放在工作线程上。 */
    static String cloudSync(String request) { return TextPolicy.utf8(cloudSyncRaw(utf8(request))); }

    /** 本地语音模型安装的进度：{@code {id, stage, downloaded, total}} 的 JSON 字节，stage 是 "download"（从本地文件安装时是 "import"）、"verify"、"extract" 或 "done"。在调用 {@link #voiceLocalModelInstall} 的工作线程上调用；不要在这里抛异常或做耗时的事。 */
    public interface LocalModelProgress {
        void onProgress(byte[] json);
    }

    /** 本地语音模型目录与安装状态：请求 {@code {"root"}}，返回 {@code {ok, value:{models:[...], default}}}。只读目录。 */
    public static String voiceLocalModels(String requestJson) {
        return TextPolicy.utf8(voiceLocalModelsRaw(utf8(requestJson)));
    }

    /** 下载、校验并发布一个本地语音模型：请求 {@code {"root","id","mirror"?}}，返回 {@code {ok, value:{path}}}。阻塞到结束，只在主进程的工作线程上调用；listener 可以为 null。 */
    public static String voiceLocalModelInstall(String requestJson, LocalModelProgress listener) {
        return TextPolicy.utf8(voiceLocalModelInstallRaw(utf8(requestJson), listener));
    }

    /** 让正在安装的模型尽快停下，安装随后以 local_model_cancelled 失败：请求 {@code {"id"}}。任何线程，立即返回。 */
    public static String voiceLocalModelCancel(String requestJson) {
        return TextPolicy.utf8(voiceLocalModelCancelRaw(utf8(requestJson)));
    }

    /** 删除一个已装的模型：请求 {@code {"root","id"}}；正在安装的模型拒绝删除。 */
    public static String voiceLocalModelRemove(String requestJson) {
        return TextPolicy.utf8(voiceLocalModelRemoveRaw(utf8(requestJson)));
    }

    /** 语音设置页的「测试」：请求 {@code {"preferences_directory","service"}}（service 是 voice.asr 或 voice.polish），按存着的配置发一次请求，返回 {@code {ok, value:{ok, message}}}，message 是给用户看的一句话，不含凭据。阻塞到约 15 秒，只在工作线程上调用。 */
    public static String testVoiceCredential(String requestJson) {
        return TextPolicy.utf8(testVoiceCredentialRaw(utf8(requestJson)));
    }

    /** 自定义翻译服务的请求描述器（DeepL 官方地址按 DeepL v2，其他地址按 DeepLX）：请求 {@code {config:{enabled,endpoint,api_key},text,source_language,target_language}}，返回 {@code {ok, value}}，value 是 {@code {url,method,headers,body,timeout_ms,max_response_bytes}}，服务关闭时为 null。描述器带着凭据，不得写进日志。不做 I/O。 */
    public static String customTranslationHttpRequest(String requestJson) {
        return TextPolicy.utf8(customTranslationHttpRequestRaw(utf8(requestJson)));
    }

    /** 小牛翻译的请求描述器：请求 {@code {config:{enabled,app_id,apikey},text,source_language,target_language,timestamp}}（timestamp 是毫秒数字符串），value 带 {@code body_utf8}，要原样发送。不做 I/O。 */
    public static String niutransTranslationHttpRequest(String requestJson) {
        return TextPolicy.utf8(niutransTranslationHttpRequestRaw(utf8(requestJson)));
    }

    /** 腾讯云机器翻译的签名描述器：请求 {@code {config:{enabled,secret_id,secret_key,region},texts,source_language,target_language,timestamp}}（timestamp 是秒），value 带签过名的 {@code body_utf8}，要逐字节原样发送。不做 I/O。 */
    public static String tencentTranslationHttpRequest(String requestJson) {
        return TextPolicy.utf8(tencentTranslationHttpRequestRaw(utf8(requestJson)));
    }

    /** 解析自定义翻译服务的回复：value 是整理过的译文字符串，没有译文或回复不认识时为 null。 */
    public static String parseCustomTranslationResponse(byte[] body) {
        return TextPolicy.utf8(parseCustomTranslationResponseRaw(body));
    }

    /** 解析小牛翻译的回复：value 是整理过的译文字符串，没有译文或回复报错时为 null。 */
    public static String parseNiuTransTranslationResponse(byte[] body) {
        return TextPolicy.utf8(parseNiuTransTranslationResponseRaw(body));
    }

    /** 解析腾讯云机器翻译的回复：value 是与请求等长的数组，没有译文的位置是 null；回复报错或条数不符时 value 为 null。{@code expected} 只接受 1 到 10。 */
    public static String parseTencentTranslationResponse(byte[] body, int expected) {
        return TextPolicy.utf8(parseTencentTranslationResponseRaw(body, expected));
    }

    /** 自定义翻译服务的回复是否表示失败（不是一个回答，哪怕是空的回答）。失败的词以后要再问，只有回答才能负缓存。 */
    public static boolean customTranslationReplyFailed(byte[] body) {
        return customTranslationReplyFailedRaw(body);
    }

    /** 小牛翻译的回复是否表示失败（限流、凭据错误、正文不认识），含义同 {@link #customTranslationReplyFailed}。 */
    public static boolean niutransTranslationReplyFailed(byte[] body) {
        return niutransTranslationReplyFailedRaw(body);
    }

    /** 自定义翻译接口地址下面的提示：value 是 {@code {issue, accepted}}，issue 是给用户看的一句话（没有问题时为空串），accepted 是偏好能否保存这个地址。不做 I/O。 */
    public static String translationEndpointIssue(String endpoint) {
        return TextPolicy.utf8(translationEndpointIssueRaw(utf8(endpoint)));
    }

    /** AI 辅助接口地址的检查：value 是 {@code {origin, problem, hint}}，origin 是 {@code ai_assistant.tokens} 里存 Token 的来源键（不能用时为 null），problem 是 null、{@code invalid} 或 {@code cleartext_public}，hint 是设置页写在地址下面的说明。不做 I/O，规则只在 Rust。 */
    public static String aiEndpointCheck(String endpoint) {
        return TextPolicy.utf8(aiEndpointCheckRaw(utf8(endpoint)));
    }

    /** 翻译服务设置页的「测试连接」：请求 {@code {service, config}}，service 是 translation.custom、translation.niutrans 或 translation.tencent，config 是表单里的值；value 是 {@code {ok, message}}，message 不含凭据和服务的回复。阻塞到约 15 秒，只在工作线程上调用。请求带着凭据，不得写进日志。 */
    public static String testApiCredential(String requestJson) {
        return TextPolicy.utf8(testApiCredentialRaw(utf8(requestJson)));
    }

    /** 只读校验固定资源包的完整内容：请求 {@code {"state_root","pack"}}，只在工作线程上调用。 */
    public static String resourcePackVerify(String requestJson) {
        return TextPolicy.utf8(resourcePackVerifyRaw(utf8(requestJson)));
    }

    /** 把升级前已解压在本机的文件收编为资源包：请求 {@code {"state_root","pack","source"}}。要哈希整组文件，只在主进程的工作线程上调用。 */
    public static String resourcePackAdopt(String requestJson) {
        return TextPolicy.utf8(resourcePackAdoptRaw(utf8(requestJson)));
    }

    /** A new on-device dictation handle; pair every one with {@link #localSpeechDestroy}. */
    public static long localSpeechCreate() {
        return localSpeechCreateRaw();
    }

    /**
     * 加载模型并开始会话，在提供音频的工作线程调用；成功返回空，失败返回原因。
     */
    public static String localSpeechStart(long handle, String modelDirectory, String language,
                                          String hotwords, int threads) {
        return localSpeechStart(handle, modelDirectory, language, hotwords, threads, 0);
    }

    /** 本地整段模型可以指定 VAD 灵敏度，0 保留原阈值；流式模型不使用此参数。 */
    public static String localSpeechStart(long handle, String modelDirectory, String language,
                                          String hotwords, int threads, int vadSensitivity) {
        NativeHandlePolicy.requirePositive(handle);
        byte[] error = localSpeechStartRaw(handle, utf8(modelDirectory), utf8(language),
            utf8(hotwords), threads, vadSensitivity);
        return error == null ? null : TextPolicy.utf8(error);
    }

    /** Feed 16 kHz mono PCM16. Returns the transcript so far when it changed, else null. Throws IllegalStateException once cancelled or on a recognizer failure. */
    public static String localSpeechAccept(long handle, short[] pcm, int count) {
        NativeHandlePolicy.requirePositive(handle);
        byte[] partial = localSpeechAcceptRaw(handle, pcm, count);
        return partial == null ? null : TextPolicy.utf8(partial);
    }

    /** 经过 VAD 的整段模型此刻是否判定在说话；流式模型始终为 false。在送音频的那个工作线程上调用。 */
    public static boolean localSpeechSpeaking(long handle) {
        NativeHandlePolicy.requirePositive(handle);
        return localSpeechSpeakingRaw(handle);
    }

    /** Flush and return the whole transcript. Throws IllegalStateException once cancelled or on failure. */
    public static String localSpeechFinish(long handle) {
        NativeHandlePolicy.requirePositive(handle);
        return TextPolicy.utf8(localSpeechFinishRaw(handle));
    }

    /** Any thread, while the handle is alive: stops a decode in progress. */
    public static void localSpeechCancel(long handle) {
        if (!NativeHandlePolicy.isOptional(handle))
            throw new IllegalArgumentException("Invalid local speech handle");
        if (handle > 0) localSpeechCancelRaw(handle);
    }

    public static void localSpeechDestroy(long handle) {
        if (!NativeHandlePolicy.isOptional(handle))
            throw new IllegalArgumentException("Invalid local speech handle");
        if (handle > 0) localSpeechDestroyRaw(handle);
    }

    /** Drop loaded models idle for `idleMillis`, or every model not in use for 0. */
    public static int localSpeechRelease(long idleMillis) {
        return localSpeechReleaseRaw(BoundsPolicy.nonNegative(idleMillis));
    }

    /**
     * Convert whole phrases with the shared OpenCC tables; return null when conversion fails.
     *
     * <p>Phrase-level conversion means 头发 becomes 頭髮 rather than 頭發.
     */
    public static String simplifiedToTraditional(String text) {
        if (text == null || text.isEmpty()) return text;
        byte[] converted = simplifiedToTraditionalRaw(text.getBytes(StandardCharsets.UTF_8));
        return converted == null ? null : TextPolicy.utf8(converted);
    }

    /** Drop the cached candidate list for this session; the next query is answered fresh. */
    public static String resetCache(long session) {
        return TextPolicy.utf8(resetCacheRaw(session));
    }
    /** Release process-level model caches after Android reports memory pressure. */
    public static void releaseProcessCache() { releaseProcessCacheRaw(); }
    /**
     * Route punctuation through the runtime so Engine commits match the keyboard state.
     *
     * <p>The Engine decides what a punctuation key produces, so changing only the key face would
     * show one mark and commit the other.
     */
    public static String setChinesePunctuation(long session, boolean enabled) {
        return TextPolicy.utf8(setChinesePunctuationRaw(session, enabled));
    }
    /**
     * Route fullwidth state through the runtime so Engine commits carry it too.
     *
     * <p>The host still widens its own commits, while Engine completions and local modes go through
     * this call.
     */
    public static String setCharacterWidth(long session, boolean fullwidth) {
        return TextPolicy.utf8(setCharacterWidthRaw(session, fullwidth));
    }
    public static String character(long session, int ascii, boolean shift) {
        if (ascii < 0 || ascii > 127) throw new IllegalArgumentException("Engine character must be ASCII");
        return TextPolicy.utf8(characterRaw(session, ascii, shift));
    }
    /** 26 键双拼的字母键几何（`msime_client_set_key_geometry`）：`json` 由 {@link TouchPointPolicy#geometryJson} 生成，空串清除。只在布局变化时调用。返回 View。 */
    public static String setKeyGeometry(long session, String json) {
        if (json == null) throw new IllegalArgumentException("Missing key geometry");
        byte[] payload = json.getBytes(StandardCharsets.UTF_8);
        if (payload.length > KEY_GEOMETRY_LIMIT) throw new IllegalArgumentException("Key geometry is too large");
        return TextPolicy.utf8(setKeyGeometryRaw(session, payload));
    }
    /** 带按下点的一键（`msime_client_touch_character`）：`x`、`y` 与注册的几何同一个坐标系，`geometryId` 是那份几何的代次。代次不符、坐标不是有限数或不是 26 键双拼时，结果与 {@link #character} 逐字节相同。返回与 {@link #character} 相同的输入响应。 */
    public static String touchCharacter(long session, int ascii, boolean shift, int geometryId, float x, float y) {
        if (ascii < 0 || ascii > 127) throw new IllegalArgumentException("Engine character must be ASCII");
        return TextPolicy.utf8(touchCharacterRaw(session, ascii, shift, geometryId, x, y));
    }
    public static String punctuationWithContext(long session, int ascii, int precedingCodePoint) {
        if (ascii < 0 || ascii > 127 || !SmartPunctuationContext.isAsciiPunctuation((char) ascii))
            throw new IllegalArgumentException("Engine punctuation must be ASCII punctuation");
        if (!Character.isValidCodePoint(precedingCodePoint)
                || (precedingCodePoint >= Character.MIN_SURROGATE
                    && precedingCodePoint <= Character.MAX_SURROGATE))
            throw new IllegalArgumentException("Preceding character must be a Unicode scalar");
        return TextPolicy.utf8(punctuationWithContextRaw(session, ascii, precedingCodePoint));
    }
    /** 宿主自己补上书名号的后半个之后通知 Engine 这一层已经闭合，下一次 `<` 才会是外层的《而不是嵌套的〈。Engine 只接受 `<`。 */
    public static String balancePairedPunctuationAfterAutoClose(long session, int opening) {
        if (opening != '<') throw new IllegalArgumentException("Only the book-title opening is balanced");
        return TextPolicy.utf8(balancePairedPunctuationAfterAutoCloseRaw(session, opening));
    }
    /** 滑行的一笔；`request` 是 `msime_client_glide` 规定的 JSON，由 {@link GlideTypingPolicy#request} 生成。 */
    public static String glide(long session, String request) {
        if (request == null) throw new IllegalArgumentException("Missing glide request");
        byte[] payload = request.getBytes(StandardCharsets.UTF_8);
        if (payload.length > GLIDE_REQUEST_LIMIT)
            throw new IllegalArgumentException("Glide request is too large");
        return TextPolicy.utf8(glideRaw(session, payload));
    }
    public static String smartPunctuationArm(long session, String request) {
        return TextPolicy.utf8(smartPunctuationArmRaw(session, boundedSmartPunctuation(request)));
    }
    public static String smartPunctuationDecide(long session, String request) {
        return TextPolicy.utf8(smartPunctuationDecideRaw(session, boundedSmartPunctuation(request)));
    }
    private static byte[] boundedSmartPunctuation(String request) {
        if (request == null) throw new IllegalArgumentException("Missing smart punctuation request");
        byte[] payload = request.getBytes(StandardCharsets.UTF_8);
        if (payload.length > SMART_PUNCTUATION_REQUEST_LIMIT)
            throw new IllegalArgumentException("Smart punctuation request is too large");
        return payload;
    }
    public static String command(long session, int command) { return TextPolicy.utf8(commandRaw(session, command)); }
    public static String select(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(selectRaw(session, generation, index));
    }
    public static String selectAnyCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(selectAnyCandidateRaw(session, generation, index));
    }
    public static String pinCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(pinCandidateRaw(session, generation, index));
    }
    public static String fixCandidatePosition(long session, long generation, long index, int position) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(fixCandidatePositionRaw(session, generation, index,
            CandidateManagementAction.validatePosition(position)));
    }
    public static String clearCandidatePosition(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(clearCandidatePositionRaw(session, generation, index));
    }
    public static String removeCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return TextPolicy.utf8(removeCandidateRaw(session, generation, index));
    }
    /**
     * 以词定字: ask the Engine for one Han character from a candidate on the current page.
     *
     * <p>Unhandled for a candidate with no Han text, which leaves the composition untouched; the
     * caller falls back to the candidate's own text, as the source's host does.
     */
    public static String selectEdge(long session, long generation, long index, int edge) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        if (edge != 0 && edge != 1) throw new IllegalArgumentException("Invalid candidate edge");
        return TextPolicy.utf8(selectEdgeRaw(session, generation, index, edge));
    }
    public static String chooseNineKeySpelling(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid nine-key spelling index");
        return TextPolicy.utf8(chooseNineKeySpellingRaw(session, generation, index));
    }
    /**
     * 全拼九键组字时的候选筛选（`msime_client_set_nine_key_filter`）：`singleCharacter` 只留单字，`strokes` 是首字的笔顺前缀（h 横、s 竖、p 撇、n 点、z 折），空串表示不按笔画筛选。返回与其他输入调用相同的响应。
     */
    public static String setNineKeyFilter(long session, boolean singleCharacter, String strokes) {
        if (strokes == null) throw new IllegalArgumentException("Missing nine-key strokes");
        byte[] payload = strokes.getBytes(StandardCharsets.UTF_8);
        if (payload.length > NineKeyPanelPolicy.MAX_STROKES)
            throw new IllegalArgumentException("Nine-key strokes are too long");
        return TextPolicy.utf8(setNineKeyFilterRaw(session, singleCharacter, payload));
    }
    public static String allCandidates(long session) { return TextPolicy.utf8(allCandidatesRaw(session)); }
    /** 座位 {@code start} 起最多 {@code count} 行候选（{@code msime_client_candidate_range}）：横向候选条每键只取它建得出来的一批，不整代复制。 */
    public static String candidateRange(long session, int start, int count) {
        if (start < 0 || count < 0) throw new IllegalArgumentException("Invalid candidate range");
        return TextPolicy.utf8(candidateRangeRaw(session, start, count));
    }
    public static String applyTranslations(long session, long generation, String translations) {
        if (generation < 0) throw new IllegalArgumentException("Invalid candidate generation");
        byte[] payload = translations.getBytes(StandardCharsets.UTF_8);
        if (payload.length > GLOSS_RESPONSE_LIMIT)
            throw new IllegalArgumentException("Candidate translations are too large");
        return TextPolicy.utf8(applyTranslationsRaw(session, generation, payload));
    }
    /** What the optional cloud and AI providers should be asked for, or null when neither applies. */
    public static String onlineQuery(long session) { return TextPolicy.utf8(onlineQueryRaw(session)); }
    /** The cloud candidate URL for a copied online query. Credentials never leave the session. */
    public static String cloudRequestUrl(String query) {
        return TextPolicy.utf8(cloudRequestUrlRaw(boundedQuery(query)));
    }
    /** A validated AI HTTPS descriptor for a copied online query, or null when it no longer applies. */
    public static String aiRequestForQuery(long session, String query) {
        return TextPolicy.utf8(aiRequestForQueryRaw(session, boundedQuery(query)));
    }
    /** 使用当前 AI 凭据构造一次候选翻译请求。 */
    public static String aiTranslationHttpRequest(String config, String text, String source,
            String target) {
        if (config == null || text == null || source == null || target == null)
            throw new IllegalArgumentException("Missing AI translation request");
        JSONObject request;
        try {
            request = new JSONObject(config)
                .put("input", new JSONObject().put("text", text)
                    .put("source_language", source).put("target_language", target));
        } catch (JSONException error) {
            throw new IllegalArgumentException("Invalid AI translation configuration", error);
        }
        return TextPolicy.utf8(aiTranslationHttpRequestRaw(request.toString().getBytes(StandardCharsets.UTF_8)));
    }
    public static String parseAiTranslationResponse(byte[] body) {
        if (body == null || body.length > GLOSS_RESPONSE_LIMIT) throw new IllegalArgumentException("AI response is too large");
        return TextPolicy.utf8(parseAiTranslationResponseRaw(body));
    }
    /** 判断 AI 翻译服务回复是否失败；合法的空译文不算失败。 */
    public static boolean aiTranslationReplyFailed(byte[] body) {
        if (body == null || body.length > GLOSS_RESPONSE_LIMIT) return true;
        return aiTranslationReplyFailedRaw(body);
    }
    public static String applyCloudResponse(long session, String query, String body) {
        byte[] payload = body.getBytes(StandardCharsets.UTF_8);
        if (payload.length > OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES)
            throw new IllegalArgumentException("Cloud response is too large");
        return TextPolicy.utf8(applyCloudResponseRaw(session, boundedQuery(query), payload));
    }
    /** Source 0 is the cloud provider and source 1 is the AI assistant. */
    public static String applyOnlineCandidates(long session, String query, String candidates,
            int source) {
        if (source != 0 && source != 1)
            throw new IllegalArgumentException("Unknown online candidate source");
        byte[] payload = candidates.getBytes(StandardCharsets.UTF_8);
        if (payload.length > ONLINE_QUERY_LIMIT)
            throw new IllegalArgumentException("Online candidates are too large");
        return TextPolicy.utf8(applyOnlineCandidatesRaw(session, boundedQuery(query), payload, source));
    }
    /** AI 回答为空、失败或被拒时调用（source 只认 1）。共享内核已经不放 AI 占位行，这个调用答 applied:false、不改候选列表；调用和接口留着不变。 */
    public static String resolveOnlineCandidates(long session, String query, int source) {
        if (source != 0 && source != 1)
            throw new IllegalArgumentException("Unknown online candidate source");
        return TextPolicy.utf8(resolveOnlineCandidatesRaw(session, boundedQuery(query), source));
    }
    /** Remove cached and visible rows for source 0 (cloud) or 1 (AI). */
    public static String clearOnlineCandidates(long session, int source) {
        if (source != 0 && source != 1)
            throw new IllegalArgumentException("Unknown online candidate source");
        return TextPolicy.utf8(clearOnlineCandidatesRaw(session, source));
    }
    private static byte[] boundedQuery(String query) {
        byte[] payload = query.getBytes(StandardCharsets.UTF_8);
        if (payload.length > ONLINE_QUERY_LIMIT)
            throw new IllegalArgumentException("Online query is too large");
        return payload;
    }
    public static String view(long session) { return TextPolicy.utf8(viewRaw(session)); }
    public static String updatePreferences(long session, String snapshot) { return TextPolicy.utf8(updatePreferencesRaw(session, snapshot.getBytes(StandardCharsets.UTF_8))); }
    public static String destroy(long session) { return TextPolicy.utf8(destroyRaw(session)); }
    private static native byte[] createRaw(byte[] options);
    private static native byte[] prepareHostRaw(byte[] options);
    private static native byte[] refreshHostRaw(byte[] path);
    private static native byte[] snapshotVersionRaw(byte[] options);
    private static native byte[] migrateKeyLayoutRaw(byte[] request);
    private static native byte[] loadPreferencesRaw(byte[] directory);
    private static native byte[] telemetryBeginRaw(byte[] request);
    private static native byte[] telemetryEndRaw(byte[] request);
    private static native byte[] telemetryFlushRaw(byte[] request);
    private static native byte[] telemetryClearRaw(byte[] request);
    private static native byte[] noticesRaw(byte[] request);
    private static native byte[] noticeDismissRaw(byte[] request);
    private static native byte[] typingStatisticsRaw(byte[] request);
    private static native int typingStatisticsEnabledRaw(byte[] directory);
    private static native byte[] themeCatalogRaw();
    private static native byte[] resolveThemeRaw(byte[] request);
    private static native byte[] skinPackageRaw(byte[] request);
    private static native byte[] skinResourceRaw(byte[] request);
    private static native byte[] appThemeCatalogRaw();
    private static native byte[] resolveAppThemeRaw(byte[] request);
    private static native byte[] restoreDefaultPreferencesRaw(byte[] directory, long expectedRevision);
    private static native byte[] defaultPreferencesRaw();
    private static native byte[] textTrackpadOptionsRaw();
    private static native byte[] hostCapabilitiesRaw(byte[] platform);
    private static native byte[] dictionaryRaw(byte[] request);
    private static native byte[] personalDictionaryRequestRaw(byte[] request);
    private static native byte[] dictionaryHansEntriesRaw(byte[] text, byte[] resources);
    private static native byte[] dictionaryImportEntriesRaw(byte[] request, byte[] resources);
    private static native byte[] dictionaryManifestRaw(byte[] resources);
    private static native byte[] pluginsRaw(byte[] request);
    private static native byte[] communityResourceLibraryRaw(byte[] request);
    private static native byte[] keyboardSkinTrialRaw(byte[] request);
    private static native byte[] communitySkinInstallRaw(byte[] request);
    private static native byte[] keySoundPackRaw(byte[] request);
    private static native byte[] worldClockRaw(byte[] request);
    private static native byte[] commonPhrasesRaw(byte[] request);
    private static native byte[] dictionaryCollectionsRaw(byte[] request);
    private static native byte[] diagnosticBundleRaw(byte[] request);
    private static native byte[] accountSettingsExportRaw(byte[] request);
    private static native byte[] accountSettingsApplyRaw(byte[] request);
    private static native byte[] vocabularyReviewRaw(byte[] request);
    private static native byte[] personalWordbookAddRaw(byte[] request);
    private static native byte[] readAloudTextRaw(byte[] request);
    private static native byte[] emojiCatalogRaw(byte[] query, byte[] resources);
    private static native byte[] candidateGlossesRaw(byte[] request, byte[] resources);
    private static native byte[] candidatePronunciationsRaw(byte[] request, byte[] resources);
    private static native byte[] englishCompletionsRaw(byte[] request, byte[] resources);
    private static native byte[] polishPromptRaw(byte[] id, byte[] custom1, byte[] custom2,
        byte[] custom3);
    private static native byte[] mobileClipboardHistoryRaw(byte[] request);
    private static native byte[] cloudClipboardE2eRaw(byte[] request);
    private static native byte[] cloudClipboardSourceRaw(byte[] directory);
    private static native byte[] cloudClipboardRelayRaw(byte[] request);
    private static native byte[] cloudClipboardRelayExchangeRaw(byte[] request);
    private static native byte[] cloudClipboardWebDavProbeRaw(byte[] request);
    private static native byte[] dictionarySyncRaw(byte[] request);
    private static native byte[] doubaoDecodeFrameRaw(byte[] frame);
    private static native byte[] doubaoStartFrameRaw(boolean itn, boolean punctuation, boolean ddc,
        byte[] boostingTableId);
    private static native byte[] doubaoAudioFrameRaw(int sequence, byte[] pcm, int length,
        boolean finalChunk);
    private static native byte[] shuangpinKeyHintsRaw(byte[] profile);
    private static native byte[] nineKeyShuangpinLayoutRaw(byte[] request);
    private static native byte[] savePreferencesRaw(byte[] directory, long expectedRevision, byte[] snapshot);
    private static native byte[] personalDictionarySyncRaw(byte[] options);
    private static native byte[] focusRaw(long session, boolean focused);
    private static native byte[] setNineKeyModeRaw(long session, boolean enabled);
    private static native byte[] setKeyGridRaw(long session, int grid);
    private static native byte[] gridKeyRaw(long session, int letter);
    private static native byte[] setPrivateSessionRaw(long session, boolean enabled);
    private static native byte[] setEnglishModeRaw(long session, boolean enabled);
    private static native byte[] mobileVoiceConfigurationRaw(byte[] directory);
    private static native byte[] mobileVoiceStatusRaw(byte[] provider, boolean resolved,
        boolean platformAvailable, boolean modelReady, boolean runtimeReady);
    private static native byte[] mobileVoicePolishStatusRaw(boolean enabled, boolean configured,
        boolean hasToken);
    private static native byte[] mobileVoicePolicyRaw(byte[] provider, byte[] authMode);
    private static native byte[] voiceProviderCatalogRaw();
    private static native byte[] aiProviderCatalogRaw();
    private static native byte[] aiProviderUpdateRaw(byte[] request);
    private static native byte[] aiModelsRaw(byte[] request);
    private static native byte[] voiceEndpointIssueRaw(byte[] endpoint, boolean websocket);
    private static native byte[] voiceCredentialIssueRaw(byte[] provider, byte[] value);
    private static native byte[] voiceWebSocketEndpointRaw(byte[] endpoint);
    private static native byte[] voiceProviderUpdateRaw(byte[] request);
    private static native byte[] simplifiedToTraditionalRaw(byte[] text);
    private static native byte[] resetCacheRaw(long session);
    private static native void releaseProcessCacheRaw();
    private static native byte[] setChinesePunctuationRaw(long session, boolean enabled);
    private static native byte[] setCharacterWidthRaw(long session, boolean fullwidth);
    private static native byte[] characterRaw(long session, int ascii, boolean shift);
    private static native byte[] setKeyGeometryRaw(long session, byte[] json);
    private static native byte[] touchCharacterRaw(long session, int ascii, boolean shift, int geometryId,
        float x, float y);
    private static native byte[] balancePairedPunctuationAfterAutoCloseRaw(long session, int opening);
    private static native byte[] punctuationWithContextRaw(long session, int ascii,
        int precedingCodePoint);
    private static native byte[] glideRaw(long session, byte[] request);
    private static native byte[] smartPunctuationArmRaw(long session, byte[] request);
    private static native byte[] smartPunctuationDecideRaw(long session, byte[] request);
    private static native byte[] commandRaw(long session, int command);
    private static native byte[] selectRaw(long session, long generation, long index);
    private static native byte[] selectAnyCandidateRaw(long session, long generation, long index);
    private static native byte[] pinCandidateRaw(long session, long generation, long index);
    private static native byte[] fixCandidatePositionRaw(long session, long generation, long index,
        int position);
    private static native byte[] clearCandidatePositionRaw(long session, long generation, long index);
    private static native byte[] removeCandidateRaw(long session, long generation, long index);
    private static native byte[] selectEdgeRaw(long session, long generation, long index, int edge);
    private static native byte[] chooseNineKeySpellingRaw(long session, long generation, long index);
    private static native byte[] setNineKeyFilterRaw(long session, boolean singleCharacter,
        byte[] strokes);
    private static native byte[] allCandidatesRaw(long session);
    private static native byte[] candidateRangeRaw(long session, int start, int count);
    private static native byte[] applyTranslationsRaw(long session, long generation,
        byte[] translations);
    private static native byte[] onlineQueryRaw(long session);
    private static native byte[] cloudRequestUrlRaw(byte[] query);
    private static native byte[] aiRequestForQueryRaw(long session, byte[] query);
    private static native byte[] aiTranslationHttpRequestRaw(byte[] request);
    private static native byte[] parseAiTranslationResponseRaw(byte[] body);
    private static native boolean aiTranslationReplyFailedRaw(byte[] body);
    private static native byte[] applyCloudResponseRaw(long session, byte[] query, byte[] body);
    private static native byte[] applyOnlineCandidatesRaw(long session, byte[] query,
        byte[] candidates, int source);
    private static native byte[] clearOnlineCandidatesRaw(long session, int source);
    private static native byte[] resolveOnlineCandidatesRaw(long session, byte[] query, int source);
    private static native byte[] viewRaw(long session);
    private static native byte[] updatePreferencesRaw(long session, byte[] snapshot);
    private static native byte[] voiceHotwordsRaw(byte[] request);
    private static native byte[] voiceHotwordCorrectRaw(byte[] request);
    private static native byte[] voiceNormalizeNumbersRaw(byte[] request);
    private static native boolean localSpeechAvailableRaw();
    private static native void localSpeechRuntimeRaw(byte[] path);
    private static native byte[] resourcePacksRaw(byte[] request);
    private static native byte[] resourcePackInstallRaw(byte[] request, ResourcePackProgress listener);
    private static native void resourcePackCancelRaw(byte[] pack);
    private static native byte[] resourcePackRemoveRaw(byte[] request);
    private static native byte[] resourcePackVerifyRaw(byte[] request);
    private static native byte[] resourcePackAdoptRaw(byte[] request);
    private static native byte[] testVoiceCredentialRaw(byte[] request);
    private static native byte[] customTranslationHttpRequestRaw(byte[] request);
    private static native byte[] niutransTranslationHttpRequestRaw(byte[] request);
    private static native byte[] tencentTranslationHttpRequestRaw(byte[] request);
    private static native byte[] parseCustomTranslationResponseRaw(byte[] body);
    private static native byte[] parseNiuTransTranslationResponseRaw(byte[] body);
    private static native byte[] parseTencentTranslationResponseRaw(byte[] body, int expected);
    private static native boolean customTranslationReplyFailedRaw(byte[] body);
    private static native boolean niutransTranslationReplyFailedRaw(byte[] body);
    private static native byte[] translationEndpointIssueRaw(byte[] endpoint);
    private static native byte[] aiEndpointCheckRaw(byte[] endpoint);
    private static native byte[] testApiCredentialRaw(byte[] request);
    private static native byte[] voiceLocalModelsRaw(byte[] request);
    private static native byte[] voiceLocalModelInstallRaw(byte[] request, LocalModelProgress listener);
    private static native byte[] voiceLocalModelCancelRaw(byte[] request);
    private static native byte[] voiceLocalModelRemoveRaw(byte[] request);
    private static native long localSpeechCreateRaw();
    private static native byte[] localSpeechStartRaw(long handle, byte[] model, byte[] language,
                                                     byte[] hotwords, int threads, int vadSensitivity);
    private static native byte[] localSpeechAcceptRaw(long handle, short[] pcm, int count);
    private static native boolean localSpeechSpeakingRaw(long handle);
    private static native byte[] localSpeechFinishRaw(long handle);
    private static native void localSpeechCancelRaw(long handle);
    private static native void localSpeechDestroyRaw(long handle);
    private static native int localSpeechReleaseRaw(long idleMillis);
    private static native byte[] destroyRaw(long session);
    // 账号与云端（#8201）：会话由 Rust 持有，经 SessionStore 注入的存储读写；登录、令牌和刷新经 AccountStore 调用，其余入口经上面各自的包装调用（sessionStoreUnregisterRaw 只有设备冒烟用）；aiSkinJobRaw、cloudDictionaryRaw 是 5a 按头文件接好的入口，产品还没有调用方，只有设备冒烟核对请求不合法码，是否删由负责人定。
    private static native byte[] sessionStoreRegisterRaw(SessionStore store, byte[] clientAgent);
    private static native byte[] sessionStoreUnregisterRaw(long sessionStore);
    private static native byte[] clientAgentRaw(byte[] request);
    private static native byte[] accountNameRaw(byte[] request);
    private static native byte[] accountRaw(byte[] request);
    private static native byte[] accountAccessTokenRaw(byte[] request);
    private static native byte[] accountCloudClipboardRaw(byte[] request);
    private static native byte[] accountChatStreamRaw(byte[] request, ChatDelta listener);
    private static native byte[] accountChatCancelRaw(byte[] request);
    private static native byte[] accountGlossRaw(byte[] request);
    private static native byte[] communityRaw(byte[] request);
    private static native byte[] aiSkinGenerateRaw(byte[] request, AiSkinProgress listener);
    private static native byte[] aiSkinCancelRaw(byte[] request);
    private static native byte[] aiSkinJobRaw(byte[] request);
    private static native byte[] cloudDictionaryRaw(byte[] request);
    private static native byte[] snapshotQueueRaw(byte[] request);
    private static native byte[] cloudSyncRaw(byte[] request);
}
