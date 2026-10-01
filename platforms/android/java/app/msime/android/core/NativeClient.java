package app.msime.android;

import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.FileInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
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
    private static final int SHUANGPIN_PROFILE_LIMIT = 64;
    private static final int SHUANGPIN_HINT_RESPONSE_LIMIT = 65_536;
    private static final int SMART_PUNCTUATION_REQUEST_LIMIT = 4_096;
    /** An imported word list is the one request here that carries a whole file. */
    private static final int VOCABULARY_REQUEST_LIMIT = 8 * 1024 * 1024;
    static { System.loadLibrary("msime_android"); }
    private NativeClient() {}
    private static String text(byte[] value) { return new String(value, StandardCharsets.UTF_8); }
    public static String create(String options) { return text(createRaw(options.getBytes(StandardCharsets.UTF_8))); }
    public static String prepareHost(String options) { return text(prepareHostRaw(options.getBytes(StandardCharsets.UTF_8))); }
    /** Runs the shared `msime_client_refresh_host` on the runtime options file at the absolute `path`. */
    public static String refreshHost(String path) { return text(refreshHostRaw(path.getBytes(StandardCharsets.UTF_8))); }
    /** Reads the current native dictionary version without creating a session. */
    public static String snapshotVersion(String options) {
        return text(snapshotVersionRaw(options.getBytes(StandardCharsets.UTF_8)));
    }
    /** Streams one NDJSON record at a time into native preparation. Call on a worker. */
    public static String snapshotPrepare(String request, String file) {
        try {
            String adjusted = replaceRecordCount(request, inspectSnapshot(file));
            return text(snapshotPrepareRaw(adjusted.getBytes(StandardCharsets.UTF_8),
                file.getBytes(StandardCharsets.UTF_8)));
        } catch (Exception error) {
            return text(snapshotPrepareRaw("{}".getBytes(StandardCharsets.UTF_8),
                "invalid".getBytes(StandardCharsets.UTF_8)));
        }
    }
    public static String snapshotDiscard(long handle) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid snapshot handle");
        return text(snapshotDiscardRaw(handle));
    }
    public static String snapshotActivate(long handle, String expectedVersion) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid snapshot handle");
        return text(snapshotActivateRaw(handle, expectedVersion.getBytes(StandardCharsets.UTF_8)));
    }
    /** May block on the shared file lock. Call on a worker, without a session handle. */
    public static String loadPreferences(String directory) { return text(loadPreferencesRaw(directory.getBytes(StandardCharsets.UTF_8))); }
    /** The global theme picker entries: ids, titles, appearance and built-in palettes, in picker order. */
    public static String themeCatalog() { return text(themeCatalogRaw()); }
    /** Resolves the colours of the selected global theme for one mode. Pure computation, safe on the main thread. */
    public static String resolveTheme(String request) { return text(resolveThemeRaw(request.getBytes(StandardCharsets.UTF_8))); }
    /** Classifies committed text and adds batched per-key press counts in native memory, and persists only aggregate counts. Call on a worker. */
    public static String typingStatistics(String request) {
        return text(typingStatisticsRaw(request.getBytes(StandardCharsets.UTF_8)));
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
        return text(vocabularyReviewRaw(bytes));
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
        return text(result);
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
        return text(result);
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
            byte[] result = englishCompletionsRaw(requestBytes, resourcesBytes);
            if (result == null || result.length > ENGLISH_COMPLETION_RESPONSE_LIMIT)
                throw new IllegalStateException("English completion response is too large");
            return text(result);
        } catch (org.json.JSONException error) {
            throw new IllegalArgumentException("Invalid English completion request", error);
        }
    }
    /**
     * The polish prompt the selected slot resolves to.
     *
     * <p>Decided by the shared C++ header the other hosts read, not by this host: the
     * slot-versus-legacy precedence has been wrong on individual hosts before, and the preset
     * bodies carry their own prompt-injection wording that must not drift between copies.
     */
    public static String polishPrompt(String id, String legacy, String custom1, String custom2,
                                      String custom3) {
        return text(polishPromptRaw(utf8(id), utf8(legacy), utf8(custom1), utf8(custom2),
                                    utf8(custom3)));
    }

    private static byte[] utf8(String value) {
        return (value == null ? "" : value).getBytes(StandardCharsets.UTF_8);
    }

    /**
     * The one clipboard history every mobile host shares.
     *
     * <p>Ordering, the fifty-entry limit, pinning and eviction are the shared store's, and the file
     * is the one the settings page reads. A second implementation here would be a second history.
     */
    public static String mobileClipboardHistory(String request) {
        return text(mobileClipboardHistoryRaw(utf8(request)));
    }

    /**
     * Decode a Doubao streaming response frame with the shared implementation.
     *
     * <p>The protocol's framing and auth modes are shared across hosts; Android supplies only the
     * transport.
     */
    public static String doubaoDecodeFrame(byte[] frame) {
        return text(doubaoDecodeFrameRaw(frame));
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
        return text(result);
    }
    /** May block on the shared file lock. Call on a worker, without a session handle. */
    public static String savePreferences(String directory, long expectedRevision, String snapshot) {
        if (expectedRevision < 0) throw new IllegalArgumentException("Invalid preferences revision");
        return text(savePreferencesRaw(directory.getBytes(StandardCharsets.UTF_8), expectedRevision,
            snapshot.getBytes(StandardCharsets.UTF_8)));
    }
    /** Applies a bounded batch of queued personal dictionary edits. Call with no active session. */
    public static String personalDictionarySync(String options) {
        return text(personalDictionarySyncRaw(options.getBytes(StandardCharsets.UTF_8)));
    }
    public static String focus(long session, boolean focused) { return text(focusRaw(session, focused)); }
    public static String setNineKeyMode(long session, boolean enabled) {
        return text(setNineKeyModeRaw(session, enabled));
    }
    public static String setEnglishMode(long session, boolean enabled) {
        return text(setEnglishModeRaw(session, enabled));
    }
    /**
     * The transcription provider and optional rewrite this device is configured for.
     *
     * <p>The same resolution the settings app performs, read from the same document. The keyboard
     * has its own voice entry and never goes through that app, so without this it would either
     * ignore the configuration or grow a second copy of the rules.
     */
    public static String mobileVoiceConfiguration(String directory) {
        return text(mobileVoiceConfigurationRaw(utf8(directory)));
    }

    /**
     * The user's own pinyin dictionary words as recognition hotwords: `{"options": HostOptions, "limit": n}` in, `{"hotwords":[{"text","pinyin"}]}` out. Reads the dictionary store, so call on a worker.
     */
    public static String voiceHotwords(String request) {
        return text(voiceHotwordsRaw(utf8(request)));
    }

    /** Pinyin-similarity hotword replacement over a final transcript: `{"text","hotwords"}` in, `{"text"}` out. Pure. */
    public static String voiceHotwordCorrect(String request) {
        return text(voiceHotwordCorrectRaw(utf8(request)));
    }

    /** Whether the packaged sherpa-onnx runtime loads. Loads it on the first call, so call on a worker. */
    public static boolean localSpeechAvailable() {
        return localSpeechAvailableRaw();
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
        if (handle == 0) throw new IllegalArgumentException("Invalid local speech handle");
        byte[] error = localSpeechStartRaw(handle, utf8(modelDirectory), utf8(language),
            utf8(hotwords), threads);
        return error == null ? null : text(error);
    }

    /** Feed 16 kHz mono PCM16. Returns the transcript so far when it changed, else null. Throws IllegalStateException once cancelled or on a recognizer failure. */
    public static String localSpeechAccept(long handle, short[] pcm, int count) {
        if (handle == 0) throw new IllegalArgumentException("Invalid local speech handle");
        byte[] partial = localSpeechAcceptRaw(handle, pcm, count);
        return partial == null ? null : text(partial);
    }

    /** Flush and return the whole transcript. Throws IllegalStateException once cancelled or on failure. */
    public static String localSpeechFinish(long handle) {
        if (handle == 0) throw new IllegalArgumentException("Invalid local speech handle");
        return text(localSpeechFinishRaw(handle));
    }

    /** Any thread, while the handle is alive: stops a decode in progress. */
    public static void localSpeechCancel(long handle) {
        if (handle != 0) localSpeechCancelRaw(handle);
    }

    public static void localSpeechDestroy(long handle) {
        if (handle != 0) localSpeechDestroyRaw(handle);
    }

    /** Drop loaded models idle for `idleMillis`, or every model not in use for 0. */
    public static int localSpeechRelease(long idleMillis) {
        return localSpeechReleaseRaw(Math.max(0, idleMillis));
    }

    /**
     * Convert whole phrases with the shared OpenCC tables; return null when conversion fails.
     *
     * <p>Phrase-level conversion means 头发 becomes 頭髮 rather than 頭發.
     */
    public static String simplifiedToTraditional(String text) {
        if (text == null || text.isEmpty()) return text;
        byte[] converted = simplifiedToTraditionalRaw(text.getBytes(StandardCharsets.UTF_8));
        return converted == null ? null : text(converted);
    }

    /** Drop the cached candidate list for this session; the next query is answered fresh. */
    public static String resetCache(long session) {
        return text(resetCacheRaw(session));
    }
    /**
     * Route punctuation through the runtime so Engine commits match the keyboard state.
     *
     * <p>The Engine decides what a punctuation key produces, so changing only the key face would
     * show one mark and commit the other.
     */
    public static String setChinesePunctuation(long session, boolean enabled) {
        return text(setChinesePunctuationRaw(session, enabled));
    }
    /**
     * Route fullwidth state through the runtime so Engine commits carry it too.
     *
     * <p>The host still widens its own commits, while Engine completions and local modes go through
     * this call.
     */
    public static String setCharacterWidth(long session, boolean fullwidth) {
        return text(setCharacterWidthRaw(session, fullwidth));
    }
    public static String character(long session, int ascii, boolean shift) {
        if (ascii < 0 || ascii > 127) throw new IllegalArgumentException("Engine character must be ASCII");
        return text(characterRaw(session, ascii, shift));
    }
    public static String punctuationWithContext(long session, int ascii, int precedingCodePoint) {
        if (ascii < 0 || ascii > 127 || !SmartPunctuationContext.isAsciiPunctuation((char) ascii))
            throw new IllegalArgumentException("Engine punctuation must be ASCII punctuation");
        if (!Character.isValidCodePoint(precedingCodePoint)
                || (precedingCodePoint >= Character.MIN_SURROGATE
                    && precedingCodePoint <= Character.MAX_SURROGATE))
            throw new IllegalArgumentException("Preceding character must be a Unicode scalar");
        return text(punctuationWithContextRaw(session, ascii, precedingCodePoint));
    }
    public static String smartPunctuationArm(long session, String request) {
        return text(smartPunctuationArmRaw(session, boundedSmartPunctuation(request)));
    }
    public static String smartPunctuationDecide(long session, String request) {
        return text(smartPunctuationDecideRaw(session, boundedSmartPunctuation(request)));
    }
    private static byte[] boundedSmartPunctuation(String request) {
        if (request == null) throw new IllegalArgumentException("Missing smart punctuation request");
        byte[] payload = request.getBytes(StandardCharsets.UTF_8);
        if (payload.length > SMART_PUNCTUATION_REQUEST_LIMIT)
            throw new IllegalArgumentException("Smart punctuation request is too large");
        return payload;
    }
    public static String command(long session, int command) { return text(commandRaw(session, command)); }
    public static String select(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(selectRaw(session, generation, index));
    }
    public static String selectAnyCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(selectAnyCandidateRaw(session, generation, index));
    }
    public static String pinCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(pinCandidateRaw(session, generation, index));
    }
    public static String fixCandidatePosition(long session, long generation, long index, int position) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(fixCandidatePositionRaw(session, generation, index,
            CandidateManagementAction.validatePosition(position)));
    }
    public static String clearCandidatePosition(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(clearCandidatePositionRaw(session, generation, index));
    }
    public static String removeCandidate(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid candidate index");
        return text(removeCandidateRaw(session, generation, index));
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
        return text(selectEdgeRaw(session, generation, index, edge));
    }
    public static String chooseNineKeySpelling(long session, long generation, long index) {
        if (index < 0) throw new IllegalArgumentException("Invalid nine-key spelling index");
        return text(chooseNineKeySpellingRaw(session, generation, index));
    }
    public static String allCandidates(long session) { return text(allCandidatesRaw(session)); }
    public static String applyTranslations(long session, long generation, String translations) {
        if (generation < 0) throw new IllegalArgumentException("Invalid candidate generation");
        byte[] payload = translations.getBytes(StandardCharsets.UTF_8);
        if (payload.length > GLOSS_RESPONSE_LIMIT)
            throw new IllegalArgumentException("Candidate translations are too large");
        return text(applyTranslationsRaw(session, generation, payload));
    }
    /** What the optional cloud and AI providers should be asked for, or null when neither applies. */
    public static String onlineQuery(long session) { return text(onlineQueryRaw(session)); }
    /** The cloud candidate URL for a copied online query. Credentials never leave the session. */
    public static String cloudRequestUrl(String query) {
        return text(cloudRequestUrlRaw(boundedQuery(query)));
    }
    /** A validated AI HTTPS descriptor for a copied online query, or null when it no longer applies. */
    public static String aiRequestForQuery(long session, String query) {
        return text(aiRequestForQueryRaw(session, boundedQuery(query)));
    }
    public static String applyCloudResponse(long session, String query, String body) {
        byte[] payload = body.getBytes(StandardCharsets.UTF_8);
        if (payload.length > OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES)
            throw new IllegalArgumentException("Cloud response is too large");
        return text(applyCloudResponseRaw(session, boundedQuery(query), payload));
    }
    /** Source 0 is the cloud provider and source 1 is the AI assistant. */
    public static String applyOnlineCandidates(long session, String query, String candidates,
            int source) {
        if (source != 0 && source != 1)
            throw new IllegalArgumentException("Unknown online candidate source");
        byte[] payload = candidates.getBytes(StandardCharsets.UTF_8);
        if (payload.length > ONLINE_QUERY_LIMIT)
            throw new IllegalArgumentException("Online candidates are too large");
        return text(applyOnlineCandidatesRaw(session, boundedQuery(query), payload, source));
    }
    private static byte[] boundedQuery(String query) {
        byte[] payload = query.getBytes(StandardCharsets.UTF_8);
        if (payload.length > ONLINE_QUERY_LIMIT)
            throw new IllegalArgumentException("Online query is too large");
        return payload;
    }
    public static String view(long session) { return text(viewRaw(session)); }
    public static String updatePreferences(long session, String snapshot) { return text(updatePreferencesRaw(session, snapshot.getBytes(StandardCharsets.UTF_8))); }
    public static String destroy(long session) { return text(destroyRaw(session)); }
    private static native byte[] createRaw(byte[] options);
    private static native byte[] prepareHostRaw(byte[] options);
    private static native byte[] refreshHostRaw(byte[] path);
    private static native byte[] snapshotVersionRaw(byte[] options);
    private static native byte[] snapshotPrepareRaw(byte[] request, byte[] file);
    private static native byte[] snapshotDiscardRaw(long handle);
    private static native byte[] snapshotActivateRaw(long handle, byte[] expectedVersion);
    private static native byte[] loadPreferencesRaw(byte[] directory);
    private static native byte[] typingStatisticsRaw(byte[] request);
    private static native int typingStatisticsEnabledRaw(byte[] directory);
    private static native byte[] themeCatalogRaw();
    private static native byte[] resolveThemeRaw(byte[] request);
    private static native byte[] vocabularyReviewRaw(byte[] request);
    private static native byte[] emojiCatalogRaw(byte[] query, byte[] resources);
    private static native byte[] candidateGlossesRaw(byte[] request, byte[] resources);
    private static native byte[] englishCompletionsRaw(byte[] request, byte[] resources);
    private static native byte[] polishPromptRaw(byte[] id, byte[] legacy, byte[] custom1,
        byte[] custom2, byte[] custom3);
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
    private static native byte[] setEnglishModeRaw(long session, boolean enabled);
    private static native byte[] mobileVoiceConfigurationRaw(byte[] directory);
    private static native byte[] simplifiedToTraditionalRaw(byte[] text);
    private static native byte[] resetCacheRaw(long session);
    private static native byte[] setChinesePunctuationRaw(long session, boolean enabled);
    private static native byte[] setCharacterWidthRaw(long session, boolean fullwidth);
    private static native byte[] characterRaw(long session, int ascii, boolean shift);
    private static native byte[] punctuationWithContextRaw(long session, int ascii,
        int precedingCodePoint);
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
    private static native byte[] allCandidatesRaw(long session);
    private static native byte[] applyTranslationsRaw(long session, long generation,
        byte[] translations);
    private static native byte[] onlineQueryRaw(long session);
    private static native byte[] cloudRequestUrlRaw(byte[] query);
    private static native byte[] aiRequestForQueryRaw(long session, byte[] query);
    private static native byte[] applyCloudResponseRaw(long session, byte[] query, byte[] body);
    private static native byte[] applyOnlineCandidatesRaw(long session, byte[] query,
        byte[] candidates, int source);
    private static native byte[] viewRaw(long session);
    private static native byte[] updatePreferencesRaw(long session, byte[] snapshot);
    private static native byte[] voiceHotwordsRaw(byte[] request);
    private static native byte[] voiceHotwordCorrectRaw(byte[] request);
    private static native boolean localSpeechAvailableRaw();
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
        try (BufferedInputStream input = new BufferedInputStream(new FileInputStream(file))) {
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
                || !footerHash.equals(hex(digest.digest())) || engineRecords > 500_000) {
            throw new IOException("invalid snapshot envelope");
        }
        return engineRecords;
    }

    private static String hex(byte[] bytes) {
        char[] digits = "0123456789abcdef".toCharArray();
        char[] output = new char[bytes.length * 2];
        for (int index = 0; index < bytes.length; index++) {
            int value = bytes[index] & 0xff;
            output[index * 2] = digits[value >>> 4];
            output[index * 2 + 1] = digits[value & 0x0f];
        }
        return new String(output);
    }
}
