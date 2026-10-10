package app.msime.android;

import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
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
    private static final int SHUANGPIN_PROFILE_LIMIT = 64;
    private static final int SHUANGPIN_HINT_RESPONSE_LIMIT = 65_536;
    private static final int SMART_PUNCTUATION_REQUEST_LIMIT = 4_096;
    private static final int GLIDE_REQUEST_LIMIT = 65_536;
    /** An imported word list is the one request here that carries a whole file. */
    private static final int VOCABULARY_REQUEST_LIMIT = 8 * 1024 * 1024;
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
    /** Streams one NDJSON record at a time into native preparation. Call on a worker. */
    public static String snapshotPrepare(String request, String file) {
        try {
            String adjusted = replaceRecordCount(request, inspectSnapshot(file));
            return TextPolicy.utf8(snapshotPrepareRaw(adjusted.getBytes(StandardCharsets.UTF_8),
                file.getBytes(StandardCharsets.UTF_8)));
        } catch (Exception error) {
            return TextPolicy.utf8(snapshotPrepareRaw("{}".getBytes(StandardCharsets.UTF_8),
                "invalid".getBytes(StandardCharsets.UTF_8)));
        }
    }
    public static String snapshotDiscard(long handle) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid snapshot handle");
        return TextPolicy.utf8(snapshotDiscardRaw(handle));
    }
    public static String snapshotActivate(long handle, String expectedVersion) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid snapshot handle");
        return TextPolicy.utf8(snapshotActivateRaw(handle, expectedVersion.getBytes(StandardCharsets.UTF_8)));
    }
    /** May block on the shared file lock. Call on a worker, without a session handle. */
    public static String loadPreferences(String directory) { return TextPolicy.utf8(loadPreferencesRaw(directory.getBytes(StandardCharsets.UTF_8))); }
    /** The global theme picker entries: ids, titles, appearance and built-in palettes, in picker order. */
    public static String themeCatalog() { return TextPolicy.utf8(themeCatalogRaw()); }
    /** Resolves the colours of the selected global theme for one mode. Pure computation, safe on the main thread. */
    public static String resolveTheme(String request) { return TextPolicy.utf8(resolveThemeRaw(request.getBytes(StandardCharsets.UTF_8))); }
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
    /** AI 设计皮肤的 compose / parse / artwork 决策；HTTP 由宿主自己发。纯计算。 */
    public static String aiSkinPlan(String request) { return TextPolicy.utf8(aiSkinPlanRaw(utf8(request))); }
    /** 皮肤试用的结束与崩溃恢复。写偏好，在工作线程调用。 */
    public static String keyboardSkinTrial(String request) { return TextPolicy.utf8(keyboardSkinTrialRaw(utf8(request))); }
    /** 社区皮肤安装并开始试用。写偏好与两个加锁文件，在工作线程调用。 */
    public static String communitySkinInstall(String request) { return TextPolicy.utf8(communitySkinInstallRaw(utf8(request))); }
    /** 一个按键音包校验后的文件清单 `{state_root,sound_packs,pack}`。读文件，不在按键路径上调用。 */
    public static String keySoundPack(String request) { return TextPolicy.utf8(keySoundPackRaw(utf8(request))); }
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
        if (bytes.length == 0 || bytes.length > VOCABULARY_REQUEST_LIMIT) {
            return "{\"ok\":false,\"error\":\"invalid vocabulary review request\"}";
        }
        return TextPolicy.utf8(vocabularyReviewRaw(bytes));
    }
    /** Reads one bounded page from the verified packaged emoji catalog. Call on a worker. */
    public static String emojiCatalog(String query, String resources) {
        byte[] queryBytes = query.getBytes(StandardCharsets.UTF_8);
        byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
        if (queryBytes.length > EMOJI_QUERY_LIMIT || resourcesBytes.length > EMOJI_RESOURCES_LIMIT)
            throw new IllegalArgumentException("Emoji catalog request is too large");
        byte[] result = emojiCatalogRaw(queryBytes, resourcesBytes);
        if (result == null || result.length > EMOJI_RESPONSE_LIMIT)
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
        if (result == null || result.length > GLOSS_RESPONSE_LIMIT)
            throw new IllegalStateException("Candidate gloss response is too large");
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
    /** Reads bounded English completions from the packaged dictionary. Call on a worker. */
    public static String englishCompletions(String prefix, String resources) {
        if (prefix == null || prefix.isEmpty() || prefix.length() > 128
                || !prefix.chars().allMatch(value -> value >= 'A' && value <= 'Z'
                    || value >= 'a' && value <= 'z'))
            throw new IllegalArgumentException("Invalid English completion prefix");
        try {
            JSONObject request = new JSONObject().put("prefix", prefix).put("limit", 12);
            byte[] requestBytes = request.toString().getBytes(StandardCharsets.UTF_8);
            byte[] resourcesBytes = resources.getBytes(StandardCharsets.UTF_8);
            if (resourcesBytes.length > ENGLISH_COMPLETION_RESOURCES_LIMIT)
                throw new IllegalArgumentException("English completion resources are too large");
            byte[] result = englishCompletionsRaw(requestBytes, resourcesBytes);
            if (result == null || result.length > ENGLISH_COMPLETION_RESPONSE_LIMIT)
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
        if (profileBytes.length == 0 || profileBytes.length > SHUANGPIN_PROFILE_LIMIT)
            throw new IllegalArgumentException("Double-pinyin profile is too large");
        byte[] result = shuangpinKeyHintsRaw(profileBytes);
        if (result == null || result.length > SHUANGPIN_HINT_RESPONSE_LIMIT)
            throw new IllegalStateException("Double-pinyin hint response is too large");
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

    /** 资源包安装的进度。在调用 {@link #resourcePackInstall} 的工作线程上调用；phase 是 "download"、"verify" 或 "done"。不要在这里抛异常或做耗时的事。 */
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

    /** 把升级前已解压在本机的文件收编为资源包：请求 {@code {"state_root","pack","source"}}。要哈希整组文件，只在主进程的工作线程上调用。 */
    public static String resourcePackAdopt(String requestJson) {
        return TextPolicy.utf8(resourcePackAdoptRaw(utf8(requestJson)));
    }

    /** A new on-device dictation handle; pair every one with {@link #localSpeechDestroy}. */
    public static long localSpeechCreate() {
        return localSpeechCreateRaw();
    }

    /**
     * Load the model and open the session. Blocks while a model loads, so call on the worker that feeds the session. Returns null on success, else the recognizer's reason.
     */
    public static String localSpeechStart(long handle, String modelDirectory, String language,
                                          String hotwords, int threads) {
        NativeHandlePolicy.requirePositive(handle);
        byte[] error = localSpeechStartRaw(handle, utf8(modelDirectory), utf8(language),
            utf8(hotwords), threads);
        return error == null ? null : TextPolicy.utf8(error);
    }

    /** Feed 16 kHz mono PCM16. Returns the transcript so far when it changed, else null. Throws IllegalStateException once cancelled or on a recognizer failure. */
    public static String localSpeechAccept(long handle, short[] pcm, int count) {
        NativeHandlePolicy.requirePositive(handle);
        byte[] partial = localSpeechAcceptRaw(handle, pcm, count);
        return partial == null ? null : TextPolicy.utf8(partial);
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
    private static native byte[] snapshotPrepareRaw(byte[] request, byte[] file);
    private static native byte[] snapshotDiscardRaw(long handle);
    private static native byte[] snapshotActivateRaw(long handle, byte[] expectedVersion);
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
    private static native byte[] appThemeCatalogRaw();
    private static native byte[] resolveAppThemeRaw(byte[] request);
    private static native byte[] restoreDefaultPreferencesRaw(byte[] directory, long expectedRevision);
    private static native byte[] defaultPreferencesRaw();
    private static native byte[] hostCapabilitiesRaw(byte[] platform);
    private static native byte[] dictionaryRaw(byte[] request);
    private static native byte[] personalDictionaryRequestRaw(byte[] request);
    private static native byte[] dictionaryHansEntriesRaw(byte[] text, byte[] resources);
    private static native byte[] dictionaryImportEntriesRaw(byte[] request, byte[] resources);
    private static native byte[] dictionaryManifestRaw(byte[] resources);
    private static native byte[] pluginsRaw(byte[] request);
    private static native byte[] communityResourceLibraryRaw(byte[] request);
    private static native byte[] aiSkinPlanRaw(byte[] request);
    private static native byte[] keyboardSkinTrialRaw(byte[] request);
    private static native byte[] communitySkinInstallRaw(byte[] request);
    private static native byte[] keySoundPackRaw(byte[] request);
    private static native byte[] commonPhrasesRaw(byte[] request);
    private static native byte[] dictionaryCollectionsRaw(byte[] request);
    private static native byte[] diagnosticBundleRaw(byte[] request);
    private static native byte[] accountSettingsExportRaw(byte[] request);
    private static native byte[] accountSettingsApplyRaw(byte[] request);
    private static native byte[] vocabularyReviewRaw(byte[] request);
    private static native byte[] emojiCatalogRaw(byte[] query, byte[] resources);
    private static native byte[] candidateGlossesRaw(byte[] request, byte[] resources);
    private static native byte[] englishCompletionsRaw(byte[] request, byte[] resources);
    private static native byte[] polishPromptRaw(byte[] id, byte[] custom1, byte[] custom2,
        byte[] custom3);
    private static native byte[] mobileClipboardHistoryRaw(byte[] request);
    private static native byte[] doubaoDecodeFrameRaw(byte[] frame);
    private static native byte[] doubaoStartFrameRaw(boolean itn, boolean punctuation, boolean ddc,
        byte[] boostingTableId);
    private static native byte[] doubaoAudioFrameRaw(int sequence, byte[] pcm, int length,
        boolean finalChunk);
    private static native byte[] shuangpinKeyHintsRaw(byte[] profile);
    private static native byte[] savePreferencesRaw(byte[] directory, long expectedRevision, byte[] snapshot);
    private static native byte[] personalDictionarySyncRaw(byte[] options);
    private static native byte[] focusRaw(long session, boolean focused);
    private static native byte[] setNineKeyModeRaw(long session, boolean enabled);
    private static native byte[] setPrivateSessionRaw(long session, boolean enabled);
    private static native byte[] setEnglishModeRaw(long session, boolean enabled);
    private static native byte[] mobileVoiceConfigurationRaw(byte[] directory);
    private static native byte[] simplifiedToTraditionalRaw(byte[] text);
    private static native byte[] resetCacheRaw(long session);
    private static native byte[] setChinesePunctuationRaw(long session, boolean enabled);
    private static native byte[] setCharacterWidthRaw(long session, boolean fullwidth);
    private static native byte[] characterRaw(long session, int ascii, boolean shift);
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
    private static native byte[] applyTranslationsRaw(long session, long generation,
        byte[] translations);
    private static native byte[] onlineQueryRaw(long session);
    private static native byte[] cloudRequestUrlRaw(byte[] query);
    private static native byte[] aiRequestForQueryRaw(long session, byte[] query);
    private static native byte[] applyCloudResponseRaw(long session, byte[] query, byte[] body);
    private static native byte[] applyOnlineCandidatesRaw(long session, byte[] query,
        byte[] candidates, int source);
    private static native byte[] clearOnlineCandidatesRaw(long session, int source);
    private static native byte[] viewRaw(long session);
    private static native byte[] updatePreferencesRaw(long session, byte[] snapshot);
    private static native byte[] voiceHotwordsRaw(byte[] request);
    private static native byte[] voiceHotwordCorrectRaw(byte[] request);
    private static native boolean localSpeechAvailableRaw();
    private static native void localSpeechRuntimeRaw(byte[] path);
    private static native byte[] resourcePacksRaw(byte[] request);
    private static native byte[] resourcePackInstallRaw(byte[] request, ResourcePackProgress listener);
    private static native void resourcePackCancelRaw(byte[] pack);
    private static native byte[] resourcePackAdoptRaw(byte[] request);
    private static native long localSpeechCreateRaw();
    private static native byte[] localSpeechStartRaw(long handle, byte[] model, byte[] language,
                                                     byte[] hotwords, int threads);
    private static native byte[] localSpeechAcceptRaw(long handle, short[] pcm, int count);
    private static native byte[] localSpeechFinishRaw(long handle);
    private static native void localSpeechCancelRaw(long handle);
    private static native void localSpeechDestroyRaw(long handle);
    private static native int localSpeechReleaseRaw(long idleMillis);
    private static native byte[] destroyRaw(long session);

    private static final Pattern TYPE = Pattern.compile("\\\"type\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"");
    private static final Pattern RECORDS = Pattern.compile("(\\\"records\\\"\\s*:\\s*)\\d+");
    private static final Pattern FOOTER_RECORDS = Pattern.compile("\\\"records\\\"\\s*:\\s*(\\d+)");
    private static final Pattern SHA256 = Pattern.compile("\\\"sha256\\\"\\s*:\\s*\\\"([0-9a-f]{64})\\\"");

    private static String replaceRecordCount(String request, int records) {
        Matcher matcher = RECORDS.matcher(request);
        if (!matcher.find()) throw new IllegalArgumentException("Snapshot request is missing records");
        return matcher.replaceFirst(Matcher.quoteReplacement(matcher.group(1) + records));
    }

    private static int inspectSnapshot(String file) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        int engineRecords = 0;
        int dataRecords = 0;
        boolean header = false;
        boolean ended = false;
        String footerHash = null;
        int footerRecords = -1;
        try (BufferedInputStream input = new BufferedInputStream(
                Files.newInputStream(Paths.get(file), LinkOption.NOFOLLOW_LINKS))) {
            ByteArrayOutputStream line = new ByteArrayOutputStream();
            int value;
            while ((value = input.read()) != -1) {
                if (value == '\n') {
                    if (line.size() == 0) throw new IOException("empty snapshot line");
                    byte[] bytes = line.toByteArray();
                    if (bytes.length > 65_535) throw new IOException("snapshot line too large");
                    String text = new String(bytes, StandardCharsets.UTF_8);
                    Matcher type = TYPE.matcher(text);
                    if (!type.find()) throw new IOException("missing snapshot type");
                    String kind = type.group(1);
                    if ("header".equals(kind)) {
                        if (header || dataRecords != 0 || ended) throw new IOException("invalid snapshot header");
                        header = true;
                        // The header counts and hashes like any other record, which is what
                        // the Server's footer was written against; skipping it rejects every
                        // valid snapshot.
                        digest.update(bytes);
                        digest.update((byte) '\n');
                        dataRecords++;
                    } else if ("footer".equals(kind)) {
                        if (!header || footerHash != null) throw new IOException("invalid snapshot footer");
                        Matcher count = FOOTER_RECORDS.matcher(text);
                        Matcher hash = SHA256.matcher(text);
                        if (!count.find() || !hash.find()) throw new IOException("invalid snapshot footer");
                        footerRecords = Integer.parseInt(count.group(1));
                        footerHash = hash.group(1);
                        ended = true;
                    } else if ("entry".equals(kind) || "overlay".equals(kind)
                            || "position".equals(kind) || "selection".equals(kind)) {
                        if (!header || ended) throw new IOException("invalid snapshot record");
                        digest.update(bytes);
                        digest.update((byte) '\n');
                        dataRecords++;
                        if (!"entry".equals(kind)) engineRecords++;
                    } else throw new IOException("unknown snapshot record");
                    line.reset();
                } else {
                    line.write(value);
                    if (line.size() > 65_535) throw new IOException("snapshot line too large");
                }
            }
            if (line.size() != 0) throw new IOException("unterminated snapshot line");
        }
        if (!header || footerHash == null || footerRecords != dataRecords
                || !footerHash.equals(DigestPolicy.hex(digest.digest())) || engineRecords > 500_000) {
            throw new IOException("invalid snapshot envelope");
        }
        return engineRecords;
    }

}
